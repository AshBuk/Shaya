// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Stealth Window - GTK4 + Layer-Shell
//!
//! Main application window with stealth features:
//! - NON_UNIQUE: No D-Bus registration (ghost mode)
//! - Lock file: Single instance without D-Bus
//! - Layer::Overlay: Hidden from wlr-screencopy (grim)
//!
//! NOTE: PipeWire capture (Zoom/Meet/OBS) sees this window.
//! See docs/STEALTH_GUIDE.md for virtual monitor workaround.

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

const APP_ID: &str = "com.shyquiet.stealth";

// Global window reference for SIGUSR1 handler
thread_local! {
    static MAIN_WINDOW: RefCell<Option<ApplicationWindow>> = const { RefCell::new(None) };
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
                    if window.is_visible() {
                        tracing::info!("SIGUSR1 received - hiding window");
                        window.set_visible(false);
                    } else {
                        tracing::info!("SIGUSR1 received - showing window");
                        window.set_visible(true);
                        window.present();
                    }
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
    let window = ApplicationWindow::builder()
        .application(app)
        .title("ShyQuiet")
        .default_width(400)
        .default_height(550)
        .build();

    // Set minimum window size (allows flexible resizing)
    window.set_size_request(300, 250);

    // Store for SIGUSR1 handler
    MAIN_WINDOW.with(|w| *w.borrow_mut() = Some(window.clone()));

    // Initialize layer-shell (stealth mode)
    layer_shell::init(&window);

    // Apply saved appearance settings
    apply_appearance_settings(&window, &state);

    // Keyboard shortcut: Ctrl+Alt+S = toggle visibility
    setup_keyboard_shortcuts(&window);

    // Build UI structure
    let main_box = GtkBox::new(Orientation::Vertical, 0);

    // Create ViewStack first (needed by header for navigation)
    let stack = build_view_stack(&window, state.clone());

    // Header bar (navigation + drag handle)
    let header = header::build(&window, &stack);
    main_box.append(&header);

    // Content area
    let content_box = GtkBox::new(Orientation::Vertical, 0);
    content_box.set_vexpand(true);
    content_box.append(&stack);
    main_box.append(&content_box);

    // Control panel (chat input)
    let control_panel = build_control_panel(&stack, state);
    main_box.append(&control_panel);

    // Wrap in overlay for resize handle
    let overlay = Overlay::new();
    overlay.set_child(Some(&main_box));

    // Resize handle (bottom-right corner)
    let resize_handle = build_resize_handle(&window);
    overlay.add_overlay(&resize_handle);

    window.set_child(Some(&overlay));
    window.present();
}

/// Build ViewStack with all views
fn build_view_stack(window: &ApplicationWindow, state: Rc<RefCell<AppState>>) -> adw::ViewStack {
    let stack = adw::ViewStack::new();
    stack.set_vexpand(true);

    // Chat view
    let chat = views::chat::build_content(state.clone());
    stack.add_titled(&chat, Some("chat"), "Chat");
    stack.page(&chat).set_icon_name(Some("chat-symbolic"));

    // Settings view (needs window reference for opacity control)
    let settings = views::settings::build(window, state.clone());
    stack.add_titled(&settings, Some("settings"), "Settings");
    stack
        .page(&settings)
        .set_icon_name(Some("emblem-system-symbolic"));

    // History view (needs stack reference for navigation)
    let history = views::history::build(state.clone(), &stack);
    stack.add_titled(&history, Some("history"), "History");
    stack
        .page(&history)
        .set_icon_name(Some("document-open-recent-symbolic"));

    // Prompts view
    let prompts = views::prompts::build(state.clone());
    stack.add_titled(&prompts, Some("prompts"), "Prompts");
    stack
        .page(&prompts)
        .set_icon_name(Some("accessories-text-editor-symbolic"));

    stack
}

/// Build control panel (chat input area)
fn build_control_panel(stack: &adw::ViewStack, state: Rc<RefCell<AppState>>) -> GtkBox {
    let panel = GtkBox::new(Orientation::Vertical, 4);
    panel.set_margin_top(4);
    panel.set_margin_bottom(8);
    panel.set_margin_start(8);
    panel.set_margin_end(8);
    panel.add_css_class("control-panel");

    let input_area = views::chat::build_input(state, stack);
    panel.append(&input_area);

    // Show only when chat view is active
    let input_c = input_area.clone();
    stack.connect_visible_child_name_notify(move |s| {
        input_c.set_visible(s.visible_child_name().as_deref() == Some("chat"));
    });

    panel
}

/// Build resize handle for bottom-right corner
fn build_resize_handle(window: &ApplicationWindow) -> Label {
    let handle = Label::new(Some("⋱"));
    handle.set_halign(gtk4::Align::End);
    handle.set_valign(gtk4::Align::End);
    handle.set_margin_end(4);
    handle.set_margin_bottom(4);
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

/// Setup keyboard shortcuts
fn setup_keyboard_shortcuts(window: &ApplicationWindow) {
    let controller = gtk4::EventControllerKey::new();
    let window_c = window.clone();

    controller.connect_key_pressed(move |_, key, _, modifiers| {
        use gtk4::gdk::{Key, ModifierType};

        // Ctrl+Alt+S = toggle visibility
        if key == Key::s
            && modifiers.contains(ModifierType::CONTROL_MASK)
            && modifiers.contains(ModifierType::ALT_MASK)
        {
            if window_c.is_visible() {
                tracing::debug!("Ctrl+Alt+S - hiding window");
                window_c.set_visible(false);
            } else {
                tracing::debug!("Ctrl+Alt+S - showing window");
                window_c.set_visible(true);
                window_c.present();
            }
            return glib::Propagation::Stop;
        }

        glib::Propagation::Proceed
    });

    window.add_controller(controller);
}
