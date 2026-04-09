// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Stealth Window - GTK4 + Layer-Shell
//!
//! Single-instance window with layer-shell namespace "shaya-stealth".
//! Configure compositor rules to control screen share visibility.
//!
//! See docs/STEALTH_GUIDE.md for setup.

use super::{header, layer_shell, views};
use crate::core::lock;
use crate::core::state::AppState;
use adw::prelude::*;
use gtk4::gio::ApplicationFlags;
use gtk4::{glib, ApplicationWindow, Box as GtkBox, GestureDrag, Label, Orientation, Overlay};
use libadwaita as adw;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

const APP_ID: &str = "com.shaya.stealth";

// Global references for SIGUSR1 handler
thread_local! {
    static MAIN_WINDOW: RefCell<Option<ApplicationWindow>> = const { RefCell::new(None) };
    static MAIN_STATE: RefCell<Option<Rc<RefCell<AppState>>>> = const { RefCell::new(None) };
    // Manual hidden state: minimize() doesn't change is_visible(), so we track it ourselves
    static WINDOW_HIDDEN: Cell<bool> = const { Cell::new(false) };
}

/// Run the stealth window application
pub fn run() -> anyhow::Result<()> {
    // Check for existing instance (no D-Bus, use lock file)
    if !lock::acquire()? {
        tracing::info!("Another instance is running, signaled to show window");
        return Ok(());
    }

    // NON_UNIQUE = no D-Bus registration = ghost mode
    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(ApplicationFlags::NON_UNIQUE)
        .build();

    app.connect_activate(|app| {
        let state = Rc::new(RefCell::new(AppState::load()));
        build_ui(app, state);
    });

    // SIGUSR1 handler: toggle window visibility
    #[cfg(unix)]
    {
        glib::unix_signal_add_local(nix::libc::SIGUSR1, || {
            MAIN_WINDOW.with(|w| {
                if let Some(window) = w.borrow().as_ref() {
                    toggle_window(window);
                }
            });
            glib::ControlFlow::Continue
        });
    }

    app.connect_shutdown(|_| lock::release());

    let exit_code = app.run();
    lock::release();

    if exit_code == glib::ExitCode::SUCCESS {
        Ok(())
    } else {
        anyhow::bail!("Application exited with error")
    }
}

/// Build the main UI
fn build_ui(app: &adw::Application, state: Rc<RefCell<AppState>>) {
    let (window_width, window_height) = {
        let s = state.borrow();
        (s.settings.window_width, s.settings.window_height)
    };
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Shaya")
        .default_width(window_width)
        .default_height(window_height)
        .build();

    // Set minimum window size (allows flexible resizing)
    window.set_size_request(300, 250);

    // Store for SIGUSR1 handler
    MAIN_WINDOW.with(|w| *w.borrow_mut() = Some(window.clone()));
    MAIN_STATE.with(|s| *s.borrow_mut() = Some(state.clone()));

    // Initialize layer-shell (stealth mode)
    layer_shell::init(&window, &state.borrow().settings);

    // Apply saved appearance settings
    apply_appearance_settings(&window, &state);

    // Build UI structure
    let main_box = GtkBox::new(Orientation::Vertical, 0);

    // Create ViewStack first (needed by header for navigation)
    let (stack, chat_shared) = build_view_stack(&window, state.clone());

    // Header bar (navigation + drag handle)
    let header = header::build(&window, &stack, state.clone());
    main_box.append(&header);

    // Content area
    let content_box = GtkBox::new(Orientation::Vertical, 0);
    content_box.set_vexpand(true);
    content_box.append(&stack);
    main_box.append(&content_box);

    // Control panel (chat input)
    let control_panel = build_control_panel(&stack, state.clone(), chat_shared);
    main_box.append(&control_panel);

    // Wrap in overlay for drag + resize handles
    let overlay = Overlay::new();
    overlay.set_child(Some(&main_box));

    // Drag handle (bottom-left)
    let drag_handle = build_drag_handle(&window, state.clone());
    overlay.add_overlay(&drag_handle);

    // Resize handle (bottom-right)
    let resize_handle = build_resize_handle(&window, state);
    overlay.add_overlay(&resize_handle);

    window.set_child(Some(&overlay));
    window.present();
}

/// Build ViewStack with all views
fn build_view_stack(
    window: &ApplicationWindow,
    state: Rc<RefCell<AppState>>,
) -> (adw::ViewStack, views::chat::ChatSharedState) {
    let stack = adw::ViewStack::new();
    stack.set_vexpand(true);

    // Chat view
    let (chat, chat_shared) = views::chat::build_content(state.clone());
    stack.add_titled(&chat, Some("chat"), "Chat");
    stack.page(&chat).set_icon_name(Some("chat-symbolic"));

    // Settings view (needs window reference for opacity control)
    let settings = views::settings::build(window, state.clone());
    stack.add_titled(&settings, Some("settings"), "Settings");
    stack
        .page(&settings)
        .set_icon_name(Some("emblem-system-symbolic"));

    // Prompts view
    let prompts = views::prompts::build(state.clone());
    stack.add_titled(&prompts, Some("prompts"), "Prompts");
    stack
        .page(&prompts)
        .set_icon_name(Some("accessories-text-editor-symbolic"));

    // History view (needs stack reference and chat_view for refresh)
    let history = views::history::build(state.clone(), &stack, &chat_shared.chat_view);
    stack.add_titled(&history, Some("history"), "History");
    stack
        .page(&history)
        .set_icon_name(Some("document-open-recent-symbolic"));

    // Cheatsheets view
    let cheatsheets = views::cheatsheets::build(state.clone());
    stack.add_titled(&cheatsheets, Some("cheatsheets"), "Cheatsheets");
    stack
        .page(&cheatsheets)
        .set_icon_name(Some("text-x-generic-symbolic"));

    // Meetings view
    let meetings = views::meetings::build(state.clone(), &stack);
    stack.add_titled(&meetings, Some("meetings"), "Meetings");
    stack
        .page(&meetings)
        .set_icon_name(Some("media-record-symbolic"));

    (stack, chat_shared)
}

/// Build control panel (chat input area)
fn build_control_panel(
    stack: &adw::ViewStack,
    state: Rc<RefCell<AppState>>,
    chat_shared: views::chat::ChatSharedState,
) -> GtkBox {
    let panel = GtkBox::new(Orientation::Vertical, 4);
    panel.set_margin_top(4);
    panel.set_margin_bottom(8);
    panel.set_margin_start(8);
    panel.set_margin_end(8);
    panel.add_css_class("control-panel");

    let input_area = views::chat::build_input(state.clone(), stack, chat_shared);
    panel.append(&input_area);

    // Show only when chat view is active
    let input_c = input_area.clone();
    stack.connect_visible_child_name_notify(move |s| {
        input_c.set_visible(s.visible_child_name().as_deref() == Some("chat"));
    });

    panel
}

/// ⋮⋮ Build drag handle (bottom-left)
fn build_drag_handle(window: &ApplicationWindow, state: Rc<RefCell<AppState>>) -> GtkBox {
    let drag_box = GtkBox::new(Orientation::Horizontal, 0);
    drag_box.set_halign(gtk4::Align::Start);
    drag_box.set_valign(gtk4::Align::End);
    drag_box.set_margin_start(8);
    drag_box.set_margin_bottom(8);
    drag_box.set_cursor_from_name(Some("grab"));

    let label = Label::new(Some("⋮⋮"));
    label.add_css_class("dim-label");
    label.set_tooltip_text(Some("Drag to move"));
    drag_box.append(&label);

    header::setup_drag_gesture(&drag_box, window, state);

    drag_box
}

/// ⤡ Build resize (bottom-right)
fn build_resize_handle(window: &ApplicationWindow, state: Rc<RefCell<AppState>>) -> Label {
    let handle = Label::new(Some("⤡"));
    handle.set_halign(gtk4::Align::End);
    handle.set_valign(gtk4::Align::End);
    handle.set_margin_end(8);
    handle.set_margin_bottom(8);
    handle.add_css_class("dim-label");
    handle.set_cursor_from_name(Some("se-resize"));
    handle.set_tooltip_text(Some("Drag to resize"));

    let start_size: Rc<Cell<(i32, i32)>> = Rc::new(Cell::new((0, 0)));

    let drag = GestureDrag::new();

    // Drag begin: store current size
    let window_c = window.clone();
    let start_size_c = start_size.clone();
    drag.connect_drag_begin(move |_, _, _| {
        let (w, h) = (window_c.width(), window_c.height());
        start_size_c.set((w, h));
    });

    // Drag update: resize window
    let window_c = window.clone();
    let start_size_c = start_size.clone();
    drag.connect_drag_update(move |_, offset_x, offset_y| {
        let (start_w, start_h) = start_size_c.get();
        let new_w = (start_w + offset_x as i32).max(300);
        let new_h = (start_h + offset_y as i32).max(250);
        window_c.set_default_size(new_w, new_h);
    });

    // Drag end: save size to state
    let window_c = window.clone();
    drag.connect_drag_end(move |_, _, _| {
        let (w, h) = (window_c.width(), window_c.height());
        let mut st = state.borrow_mut();
        st.settings.window_width = w;
        st.settings.window_height = h;
        if let Err(e) = st.save() {
            tracing::warn!("Failed to save window size: {}", e);
        }
    });

    handle.add_controller(drag);
    handle
}

/// Apply appearance settings from saved state
fn apply_appearance_settings(window: &ApplicationWindow, state: &Rc<RefCell<AppState>>) {
    let settings = &state.borrow().settings;

    // Apply opacity
    window.set_opacity(settings.window_opacity);

    // Apply dark mode
    if let Some(display) = gtk4::gdk::Display::default() {
        let style_manager = adw::StyleManager::for_display(&display);
        style_manager.set_color_scheme(if settings.dark_mode {
            adw::ColorScheme::ForceDark
        } else {
            adw::ColorScheme::ForceLight
        });
    }
}

/// Hide window (layer-shell and GNOME both use set_visible)
pub(crate) fn hide_window(window: &ApplicationWindow) {
    WINDOW_HIDDEN.with(|h| h.set(true));
    tracing::info!("Hiding window");
    window.set_visible(false);
}

/// Show window: layer-shell re-applies margins, GNOME relies on compositor positioning
fn show_window(window: &ApplicationWindow) {
    WINDOW_HIDDEN.with(|h| h.set(false));
    tracing::info!("Showing window");
    window.set_visible(true);

    // Layer-shell: re-apply margins before present to avoid position flicker
    #[cfg(target_os = "linux")]
    {
        use gtk4_layer_shell::LayerShell;
        if window.is_layer_window() {
            MAIN_STATE.with(|s| {
                if let Some(state) = s.borrow().as_ref() {
                    use gtk4_layer_shell::Edge;
                    let st = state.borrow();
                    window.set_margin(Edge::Left, st.settings.window_x);
                    window.set_margin(Edge::Top, st.settings.window_y);
                }
            });
        }
    }

    window.present();
}

/// Toggle window visibility (called by SIGUSR1 on re-run)
fn toggle_window(window: &ApplicationWindow) {
    let hidden = WINDOW_HIDDEN.with(|h| h.get());
    if hidden {
        show_window(window);
    } else {
        hide_window(window);
    }
}
