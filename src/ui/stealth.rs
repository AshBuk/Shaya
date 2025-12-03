// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Stealth Window - GTK4 + Layer-Shell
//!
//! Unified invisible overlay with input passthrough on content area.
//! Only control panel and drag handle receive input events.
//!
//! STEALTH FEATURES:
//! - NON_UNIQUE: No D-Bus registration (ghost mode)
//! - Lock file: Prevents duplicate instances without D-Bus
//! - Layer::Overlay: Invisible in screen share

use crate::core::lock;
use crate::core::state::AppState;
use crate::ui::views;
use adw::prelude::*;
use gtk4::gio::ApplicationFlags;
use gtk4::{
    glib, ApplicationWindow, Box as GtkBox, EventControllerMotion, GestureDrag, Orientation,
    ToggleButton,
};
use libadwaita as adw;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const APP_ID: &str = "com.shyquiet.stealth";

// Global window reference for signal handler
thread_local! {
    static MAIN_WINDOW: RefCell<Option<ApplicationWindow>> = const { RefCell::new(None) };
}

/// Run the stealth window application
pub fn run() -> anyhow::Result<()> {
    // STEALTH: Check for existing instance via lock file (no D-Bus)
    // Returns false if another instance is running (signaled to show)
    if !lock::acquire()? {
        tracing::info!("Another instance is running, signaled to show window");
        return Ok(());
    }

    // STEALTH: NON_UNIQUE = no D-Bus registration = true ghost mode
    // The app won't appear in D-Bus service list or respond to activation
    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(ApplicationFlags::NON_UNIQUE)
        .build();

    app.connect_activate(|app| {
        // Load state from disk
        let state = Rc::new(RefCell::new(AppState::load()));
        build_ui(app, state);
    });

    // Setup SIGUSR1 handler to show window when another instance is launched
    #[cfg(unix)]
    {
        glib::unix_signal_add_local(nix::libc::SIGUSR1, || {
            tracing::info!("SIGUSR1 received - showing window");
            MAIN_WINDOW.with(|w| {
                if let Some(window) = w.borrow().as_ref() {
                    window.set_visible(true);
                    window.present();
                }
            });
            glib::ControlFlow::Continue
        });
    }

    // Clean up lock file on shutdown
    app.connect_shutdown(|_| {
        lock::release();
    });

    let exit_code = app.run();

    // Also release on normal exit (in case shutdown signal wasn't called)
    lock::release();

    if exit_code == glib::ExitCode::SUCCESS {
        Ok(())
    } else {
        anyhow::bail!("Application exited with error")
    }
}

fn build_ui(app: &adw::Application, state: Rc<RefCell<AppState>>) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("ShyQuiet")
        .default_width(400)
        .default_height(550)
        .build();

    // Store window reference for SIGUSR1 handler
    MAIN_WINDOW.with(|w| {
        *w.borrow_mut() = Some(window.clone());
    });

    // Initialize layer-shell for stealth mode
    #[cfg(target_os = "linux")]
    init_layer_shell(&window);

    // Keyboard shortcut: Ctrl+Alt+S = toggle visibility (hide/show)
    // When hidden, use ./dev run to show (sends SIGUSR1)
    let key_controller = gtk4::EventControllerKey::new();
    let window_c = window.clone();
    key_controller.connect_key_pressed(move |_, key, _, modifiers| {
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
            return gtk4::glib::Propagation::Stop;
        }

        gtk4::glib::Propagation::Proceed
    });

    window.add_controller(key_controller);

    // Main container
    let main_box = GtkBox::new(Orientation::Vertical, 0);

    // ViewStack for different views (created first for header navigation)
    let stack = adw::ViewStack::new();
    stack.set_vexpand(true);

    // Chat view (read-only content)
    let chat_content = views::chat::build_content(state.clone());
    stack.add_titled(&chat_content, Some("chat"), "Chat");
    stack
        .page(&chat_content)
        .set_icon_name(Some("chat-symbolic"));

    // Settings view
    let settings_view = views::settings::build(state.clone());
    stack.add_titled(&settings_view, Some("settings"), "Settings");
    stack
        .page(&settings_view)
        .set_icon_name(Some("emblem-system-symbolic"));

    // History view
    let history_view = views::history::build(state.clone());
    stack.add_titled(&history_view, Some("history"), "History");
    stack
        .page(&history_view)
        .set_icon_name(Some("document-open-recent-symbolic"));

    // Prompts view
    let prompts_view = views::prompts::build(state.clone());
    stack.add_titled(&prompts_view, Some("prompts"), "Prompts");
    stack
        .page(&prompts_view)
        .set_icon_name(Some("accessories-text-editor-symbolic"));

    // ═══════════════════════════════════════════════════════════════════
    // HEADER BAR - drag handle + navigation
    // ═══════════════════════════════════════════════════════════════════
    let header = build_header(&window, &stack);
    main_box.append(&header);

    // ═══════════════════════════════════════════════════════════════════
    // CONTENT AREA
    // ═══════════════════════════════════════════════════════════════════
    let content_box = GtkBox::new(Orientation::Vertical, 0);
    content_box.set_vexpand(true);
    content_box.append(&stack);
    main_box.append(&content_box);

    // ═══════════════════════════════════════════════════════════════════
    // CONTROL PANEL - chat input + action buttons
    // ═══════════════════════════════════════════════════════════════════
    let control_panel = build_control_panel(&stack, state.clone());
    main_box.append(&control_panel);

    window.set_child(Some(&main_box));
    window.present();
}

/// Header bar with drag handle and navigation
/// Draggable area + view switcher buttons
fn build_header(window: &ApplicationWindow, stack: &adw::ViewStack) -> GtkBox {
    let header = GtkBox::new(Orientation::Horizontal, 4);
    header.set_margin_top(4);
    header.set_margin_bottom(4);
    header.set_margin_start(8);
    header.set_margin_end(8);
    header.add_css_class("header-bar");

    // Drag indicator (left)
    let drag_icon = gtk4::Label::new(Some("⋮⋮"));
    drag_icon.add_css_class("dim-label");
    drag_icon.set_tooltip_text(Some("Drag to move window"));
    header.append(&drag_icon);

    // Spacer
    let spacer_left = GtkBox::new(Orientation::Horizontal, 0);
    spacer_left.set_hexpand(true);
    header.append(&spacer_left);

    // Navigation buttons (center)
    let nav = GtkBox::new(Orientation::Horizontal, 2);
    nav.add_css_class("linked");

    let chat_btn = ToggleButton::with_label("💬");
    chat_btn.set_tooltip_text(Some("Chat"));
    chat_btn.set_active(true);

    let settings_btn = ToggleButton::with_label("⚙️");
    settings_btn.set_tooltip_text(Some("Settings"));
    settings_btn.set_group(Some(&chat_btn));

    let history_btn = ToggleButton::with_label("📜");
    history_btn.set_tooltip_text(Some("History"));
    history_btn.set_group(Some(&chat_btn));

    let prompts_btn = ToggleButton::with_label("📝");
    prompts_btn.set_tooltip_text(Some("Prompts"));
    prompts_btn.set_group(Some(&chat_btn));

    // Connect navigation to stack
    let stack_c = stack.clone();
    chat_btn.connect_toggled(move |btn| {
        if btn.is_active() {
            stack_c.set_visible_child_name("chat");
        }
    });

    let stack_c = stack.clone();
    settings_btn.connect_toggled(move |btn| {
        if btn.is_active() {
            stack_c.set_visible_child_name("settings");
        }
    });

    let stack_c = stack.clone();
    history_btn.connect_toggled(move |btn| {
        if btn.is_active() {
            stack_c.set_visible_child_name("history");
        }
    });

    let stack_c = stack.clone();
    prompts_btn.connect_toggled(move |btn| {
        if btn.is_active() {
            stack_c.set_visible_child_name("prompts");
        }
    });

    nav.append(&chat_btn);
    nav.append(&settings_btn);
    nav.append(&history_btn);
    nav.append(&prompts_btn);
    header.append(&nav);

    // Spacer
    let spacer_right = GtkBox::new(Orientation::Horizontal, 0);
    spacer_right.set_hexpand(true);
    header.append(&spacer_right);

    // Hide button
    let hide_btn = gtk4::Button::with_label("—");
    hide_btn.set_tooltip_text(Some("Hide (Ctrl+Alt+S or ./dev run to show)"));
    hide_btn.add_css_class("flat");
    let window_c = window.clone();
    hide_btn.connect_clicked(move |_| {
        tracing::debug!("Hide button clicked");
        window_c.set_visible(false);
    });
    header.append(&hide_btn);

    // Drag indicator (right)
    let drag_icon_r = gtk4::Label::new(Some("⋮⋮"));
    drag_icon_r.add_css_class("dim-label");
    header.append(&drag_icon_r);

    // ═══════════════════════════════════════════════════════════════════
    // DRAG GESTURE - entire header is draggable
    // ═══════════════════════════════════════════════════════════════════
    let start_pos: Rc<Cell<Option<(f64, f64)>>> = Rc::new(Cell::new(None));
    let window_pos: Rc<Cell<(i32, i32)>> = Rc::new(Cell::new((0, 0)));

    let drag = GestureDrag::new();

    let window_c = window.clone();
    let start_pos_c = start_pos.clone();
    let window_pos_c = window_pos.clone();
    drag.connect_drag_begin(move |_, x, y| {
        start_pos_c.set(Some((x, y)));

        // Only use layer-shell margins if window is a layer surface
        #[cfg(target_os = "linux")]
        {
            use gtk4_layer_shell::{Edge, LayerShell};
            if window_c.is_layer_window() {
                let margin_left = window_c.margin(Edge::Left);
                let margin_top = window_c.margin(Edge::Top);
                window_pos_c.set((margin_left, margin_top));
            }
        }
    });

    let window_c = window.clone();
    let start_pos_c = start_pos.clone();
    let window_pos_c = window_pos.clone();
    drag.connect_drag_update(move |_, offset_x, offset_y| {
        if start_pos_c.get().is_some() {
            let (start_x, start_y) = window_pos_c.get();
            let new_x = start_x + offset_x as i32;
            let new_y = start_y + offset_y as i32;

            // Only use layer-shell margins if window is a layer surface
            #[cfg(target_os = "linux")]
            {
                use gtk4_layer_shell::{Edge, LayerShell};
                if window_c.is_layer_window() {
                    window_c.set_margin(Edge::Left, new_x);
                    window_c.set_margin(Edge::Top, new_y);
                }
            }
        }
    });

    let start_pos_c = start_pos.clone();
    drag.connect_drag_end(move |_, _, _| {
        start_pos_c.set(None);
    });

    header.add_controller(drag);

    // Visual feedback on hover
    let motion = EventControllerMotion::new();
    let header_c = header.clone();
    motion.connect_enter(move |_, _, _| {
        header_c.add_css_class("header-bar-hover");
    });
    let header_c = header.clone();
    motion.connect_leave(move |_| {
        header_c.remove_css_class("header-bar-hover");
    });
    header.add_controller(motion);

    header
}

/// Control panel with chat input and action buttons
/// Navigation moved to header bar
fn build_control_panel(stack: &adw::ViewStack, state: Rc<RefCell<AppState>>) -> GtkBox {
    let panel = GtkBox::new(Orientation::Vertical, 4);
    panel.set_margin_top(4);
    panel.set_margin_bottom(8);
    panel.set_margin_start(8);
    panel.set_margin_end(8);
    panel.add_css_class("control-panel");

    // Chat input area (text entry + action buttons)
    let input_area = views::chat::build_input(state.clone(), stack);
    panel.append(&input_area);

    // Show/hide input area based on active view
    // Only show when chat view is active
    let input_c = input_area.clone();
    stack.connect_visible_child_name_notify(move |stack| {
        let is_chat = stack.visible_child_name().as_deref() == Some("chat");
        input_c.set_visible(is_chat);
    });

    panel
}

/// Initialize layer-shell for stealth mode
#[cfg(target_os = "linux")]
fn init_layer_shell(window: &ApplicationWindow) {
    use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

    // Early check: is this even Wayland?
    let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    if session_type != "wayland" {
        tracing::warn!("Not running on Wayland (XDG_SESSION_TYPE={})", session_type);
        tracing::warn!("Stealth mode requires Wayland with wlr-layer-shell support");
        return;
    }

    // Check desktop environment - GNOME doesn't support wlr-layer-shell
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    if desktop.to_lowercase().contains("gnome") {
        tracing::warn!("Running on GNOME - stealth mode unavailable");
        tracing::warn!("GNOME does not support wlr-layer-shell protocol");
        tracing::warn!(
            "Stealth mode works on: Sway, Hyprland, and other wlroots-based compositors"
        );
        tracing::info!("ShyQuiet will run as a normal window (visible in screen share)");
        return;
    }

    // Check if layer-shell is supported (Wayland with wlr-layer-shell)
    if !gtk4_layer_shell::is_supported() {
        tracing::warn!("Layer-shell not supported - stealth mode unavailable");
        tracing::warn!(
            "Make sure you're running on Wayland with a compositor that supports wlr-layer-shell"
        );
        return;
    }

    window.init_layer_shell();

    // Layer::Overlay — invisible in screen share (stealth mode requirement)
    // This is the highest layer, not captured by grim/screenshot tools
    window.set_layer(Layer::Overlay);

    // KeyboardMode::OnDemand — get keyboard focus only when widgets need it
    window.set_keyboard_mode(KeyboardMode::OnDemand);

    // Exclusive zone -1: don't reserve screen space
    window.set_exclusive_zone(-1);

    // Floating window (no anchors = can be positioned anywhere)
    window.set_anchor(Edge::Left, false);
    window.set_anchor(Edge::Right, false);
    window.set_anchor(Edge::Top, false);
    window.set_anchor(Edge::Bottom, false);

    window.set_namespace("shyquiet-stealth");

    tracing::info!("Stealth mode enabled (layer-shell: Overlay, keyboard=OnDemand, exclusive=-1)");
}
