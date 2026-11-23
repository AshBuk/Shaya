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
//! See docs/OVERLAY_GUIDE.md for setup.

use super::{header, layer_shell, theme, views};
use crate::core::lock;
use crate::core::state::AppState;
use adw::prelude::*;
use gtk4::gio::{self, ApplicationFlags};
use gtk4::{glib, ApplicationWindow, Box as GtkBox, GestureDrag, Orientation, Overlay};
use libadwaita as adw;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

const APP_ID: &str = "io.github.ashbuk.shaya";

// Global references for SIGUSR1 handler
thread_local! {
    static MAIN_WINDOW: RefCell<Option<ApplicationWindow>> = const { RefCell::new(None) };
    static MAIN_STATE: RefCell<Option<Rc<RefCell<AppState>>>> = const { RefCell::new(None) };
    static WINDOW_HIDDEN: Cell<bool> = const { Cell::new(false) };
}

/// Run the stealth window application
pub fn run() -> anyhow::Result<()> {
    // Check for existing instance (no D-Bus, use lock file)
    let Some(_instance_lock) = lock::acquire()? else {
        tracing::info!("Another instance is running, signaled to show window");
        return Ok(());
    };

    // NON_UNIQUE: no D-Bus name, the file lock keeps a single instance
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

    let exit_code = app.run();

    if exit_code == glib::ExitCode::SUCCESS {
        Ok(())
    } else {
        anyhow::bail!("Application exited with error")
    }
}

/// Build the main UI
fn build_ui(app: &adw::Application, state: Rc<RefCell<AppState>>) {
    theme::init(&state.borrow().settings);

    let (window_width, window_height) = {
        let s = state.borrow();
        (s.settings.window_width, s.settings.window_height)
    };
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Shaya")
        .default_width(window_width)
        .default_height(window_height)
        .css_classes(["shaya"])
        .build();

    // Set minimum window size (allows flexible resizing)
    window.set_size_request(300, 250);

    // Store for SIGUSR1 handler
    MAIN_WINDOW.with(|w| *w.borrow_mut() = Some(window.clone()));
    MAIN_STATE.with(|s| *s.borrow_mut() = Some(state.clone()));

    // Initialize layer-shell (stealth mode)
    layer_shell::init(&window, &state.borrow().settings);
    install_window_actions(app, &window);

    // Apply saved appearance settings
    apply_color_scheme(state.borrow().settings.dark_mode);

    // The overlay is a transparent layer surface: it draws its own frame,
    // moves and resizes itself and can collapse to the orb. A normal window
    // (GNOME, X11) leaves all of that to the window manager.
    let overlay_mode = is_layer_window(&window);

    // Build UI structure
    let main_box = GtkBox::new(Orientation::Vertical, 0);
    main_box.add_css_class("shaya-surface");
    main_box.set_overflow(gtk4::Overflow::Hidden);

    // Create ViewStack first (needed by header for navigation)
    let (stack, chat_shared) = build_view_stack(state.clone());

    // Header bar: inside the overlay panel, or the titlebar of a normal window
    let header = header::build(&window, &stack, state.clone(), overlay_mode);
    if overlay_mode {
        window.add_css_class("overlay");
        main_box.add_css_class("floating");
        main_box.append(&header);
    } else {
        window.set_titlebar(Some(&gtk4::WindowHandle::builder().child(&header).build()));
    }

    // Content area
    let content_box = GtkBox::new(Orientation::Vertical, 0);
    content_box.set_vexpand(true);
    content_box.append(&stack);
    main_box.append(&content_box);

    // Control panel (chat input)
    let control_panel = build_control_panel(&window, &stack, state.clone(), chat_shared);
    main_box.append(&control_panel);

    if overlay_mode {
        // Wrap in overlay for the resize handle
        let overlay = Overlay::new();
        overlay.set_child(Some(&main_box));
        overlay.add_overlay(&build_resize_handle(&window, state.clone()));
        super::orbit::install(&window, &overlay, &stack, state);
    } else {
        window.set_child(Some(&main_box));
    }
    window.present();
}

/// Hide and quit, for the header and the menu.
fn install_window_actions(app: &adw::Application, window: &ApplicationWindow) {
    let hide = gio::SimpleAction::new("hide", None);
    let window_c = window.clone();
    hide.connect_activate(move |_, _| hide_window(&window_c));
    window.add_action(&hide);

    let quit = gio::SimpleAction::new("quit", None);
    let app_c = app.clone();
    quit.connect_activate(move |_, _| app_c.quit());
    window.add_action(&quit);
    app.set_accels_for_action("win.quit", &["<Control>q"]);

    // A normal window (GNOME, X11) hides on close like the overlay; Quit ends the app.
    window.connect_close_request(|window| {
        hide_window(window);
        glib::Propagation::Stop
    });
}

/// Build ViewStack with all views. Its pages, in this order, are the sections
/// every navigation shows (see `header::sections`).
fn build_view_stack(
    state: Rc<RefCell<AppState>>,
) -> (adw::ViewStack, views::chat::ChatSharedState) {
    let stack = adw::ViewStack::new();
    stack.set_vexpand(true);
    let add = |child: &gtk4::Widget, name: &str, title: &str, icon: &str| {
        stack
            .add_titled(child, Some(name), title)
            .set_icon_name(Some(icon));
    };

    let (chat, chat_shared) = views::chat::build_content(state.clone());
    add(chat.upcast_ref(), "chat", "Chat", "go-home-symbolic");

    let meetings = views::meetings::build(state.clone(), &stack);
    add(
        meetings.upcast_ref(),
        "meetings",
        "Meetings",
        "media-record-symbolic",
    );

    // History needs the transcript to refresh it
    let history = views::history::build(state.clone(), &stack, &chat_shared.transcript);
    add(
        history.upcast_ref(),
        "history",
        "History",
        "document-open-recent-symbolic",
    );

    let prompts = views::prompts::build(state.clone());
    add(
        prompts.upcast_ref(),
        "prompts",
        "Prompts",
        "accessories-text-editor-symbolic",
    );

    let cheatsheets = views::cheatsheets::build(state.clone());
    add(
        cheatsheets.upcast_ref(),
        "cheatsheets",
        "Notes",
        "text-x-generic-symbolic",
    );

    let settings = views::settings::build(state.clone());
    add(
        settings.upcast_ref(),
        "settings",
        "Settings",
        "emblem-system-symbolic",
    );

    (stack, chat_shared)
}

/// Build control panel (chat input area)
fn build_control_panel(
    window: &ApplicationWindow,
    stack: &adw::ViewStack,
    state: Rc<RefCell<AppState>>,
    chat_shared: views::chat::ChatSharedState,
) -> GtkBox {
    let panel = GtkBox::new(Orientation::Vertical, 0);
    panel.set_margin_top(4);
    panel.set_margin_bottom(10);
    panel.set_margin_start(10);
    panel.set_margin_end(10);

    panel.append(&views::chat::build_input(window, state, stack, chat_shared));

    // Show only when chat view is active
    let panel_c = panel.clone();
    stack.connect_visible_child_name_notify(move |s| {
        panel_c.set_visible(s.visible_child_name().as_deref() == Some("chat"));
    });

    panel
}

/// Resize grip in the bottom-right corner of the overlay
fn build_resize_handle(window: &ApplicationWindow, state: Rc<RefCell<AppState>>) -> GtkBox {
    let handle = GtkBox::new(Orientation::Horizontal, 0);
    handle.add_css_class("resize-grip");
    handle.set_halign(gtk4::Align::End);
    handle.set_valign(gtk4::Align::End);
    handle.set_cursor_from_name(Some("se-resize"));
    handle.set_tooltip_text(Some("Drag to resize"));

    let start_size: Rc<Cell<(i32, i32)>> = Rc::new(Cell::new((0, 0)));

    let drag = GestureDrag::new();

    let window_c = window.clone();
    let start_size_c = start_size.clone();
    drag.connect_drag_begin(move |_, _, _| {
        start_size_c.set((window_c.width(), window_c.height()));
    });

    let window_c = window.clone();
    drag.connect_drag_update(move |_, offset_x, offset_y| {
        let (start_w, start_h) = start_size.get();
        let new_w = (start_w + offset_x as i32).max(300);
        let new_h = (start_h + offset_y as i32).max(250);
        window_c.set_default_size(new_w, new_h);
    });

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

/// Apply the saved color scheme
fn apply_color_scheme(is_dark: bool) {
    if let Some(display) = gtk4::gdk::Display::default() {
        let style_manager = adw::StyleManager::for_display(&display);
        style_manager.set_color_scheme(if is_dark {
            adw::ColorScheme::ForceDark
        } else {
            adw::ColorScheme::ForceLight
        });
    }
}

/// Hide window (set_visible works on both layer-shell and GNOME)
fn hide_window(window: &ApplicationWindow) {
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
                    let st = state.borrow();
                    layer_shell::place(window, st.settings.window_x, st.settings.window_y);
                }
            });
        }
    }

    window.present();
}

/// Toggle window visibility (called by SIGUSR1 on re-run)
fn toggle_window(window: &ApplicationWindow) {
    let hidden = WINDOW_HIDDEN.with(|h| h.get());
    // The overlay stays above everything. A normal window can sit behind others,
    // so the first press brings it up and the next one hides it.
    let behind = !is_layer_window(window) && !window.is_active();
    if hidden || behind {
        show_window(window);
    } else {
        hide_window(window);
    }
}

fn is_layer_window(window: &ApplicationWindow) -> bool {
    #[cfg(target_os = "linux")]
    {
        use gtk4_layer_shell::LayerShell;
        window.is_layer_window()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = window;
        false
    }
}
