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
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

const APP_ID: &str = "com.shyquiet.stealth";

/// Get lock file path
fn lock_file_path() -> PathBuf {
    dirs::runtime_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("shyquiet.lock")
}

/// Acquire lock file (returns Ok if acquired, Err if already running)
fn acquire_lock() -> anyhow::Result<()> {
    let lock_path = lock_file_path();

    if lock_path.exists() {
        // Check if the process is still running by reading PID
        if let Ok(pid_str) = fs::read_to_string(&lock_path) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                // Check if process exists
                let proc_path = format!("/proc/{}", pid);
                if std::path::Path::new(&proc_path).exists() {
                    anyhow::bail!("ShyQuiet is already running (PID: {})", pid);
                }
            }
        }
        // Stale lock file, remove it
        let _ = fs::remove_file(&lock_path);
    }

    // Create lock file with our PID
    fs::write(&lock_path, std::process::id().to_string())?;
    tracing::debug!("Lock file acquired: {:?}", lock_path);
    Ok(())
}

/// Release lock file
fn release_lock() {
    let lock_path = lock_file_path();
    if let Err(e) = fs::remove_file(&lock_path) {
        tracing::debug!("Failed to remove lock file: {}", e);
    }
}

/// Run the stealth window application
pub fn run() -> anyhow::Result<()> {
    // STEALTH: Check for existing instance via lock file (no D-Bus)
    acquire_lock()?;

    // STEALTH: NON_UNIQUE = no D-Bus registration = true ghost mode
    // The app won't appear in D-Bus service list or respond to activation
    // Using AdwApplication instead of gtk4::Application for better libadwaita integration
    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(ApplicationFlags::NON_UNIQUE)
        .build();

    app.connect_activate(|app| {
        // Load state from disk
        let state = Rc::new(RefCell::new(AppState::load()));
        build_ui(app, state);
    });

    // Clean up lock file on shutdown
    app.connect_shutdown(|_| {
        release_lock();
    });

    let exit_code = app.run();

    // Also release on normal exit (in case shutdown signal wasn't called)
    release_lock();

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

    // Initialize layer-shell for stealth mode
    #[cfg(target_os = "linux")]
    init_layer_shell(&window);

    // Prevent window from hiding on focus loss
    // Some compositors hide layer-shell windows when they lose focus
    {
        let window_c = window.clone();
        window.connect_is_active_notify(move |win| {
            let is_active = win.is_active();
            tracing::debug!("Window active state changed: {}", is_active);

            if !is_active {
                // Force window to stay visible by presenting it again
                // Use idle_add to avoid recursion
                let win_c = window_c.clone();
                glib::idle_add_local_once(move || {
                    if win_c.is_visible() {
                        tracing::debug!("Re-presenting window after focus loss");
                        win_c.present();
                    }
                });
            }
        });
    }

    // Main container
    let main_box = GtkBox::new(Orientation::Vertical, 0);

    // ═══════════════════════════════════════════════════════════════════
    // DRAG HANDLE - accepts input for window movement
    // ═══════════════════════════════════════════════════════════════════
    let drag_handle = build_drag_handle(&window);
    main_box.append(&drag_handle);

    // ═══════════════════════════════════════════════════════════════════
    // CONTENT AREA - passthrough (set_can_target = false)
    // ═══════════════════════════════════════════════════════════════════
    let content_box = GtkBox::new(Orientation::Vertical, 0);
    content_box.set_vexpand(true);

    // ViewStack for different views
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

    content_box.append(&stack);

    // Note: We don't set can_target(false) on content_box because it blocks
    // scroll events from reaching ScrolledWindow children.
    // Instead, we rely on layer-shell overlay behavior for stealth.

    main_box.append(&content_box);

    // ═══════════════════════════════════════════════════════════════════
    // CONTROL PANEL - accepts input (navigation, actions)
    // ═══════════════════════════════════════════════════════════════════
    let control_panel = build_control_panel(&stack, state.clone());
    main_box.append(&control_panel);

    window.set_child(Some(&main_box));
    window.present();
}

/// Drag handle for window movement
/// This is the only way to move the window on Wayland
fn build_drag_handle(window: &ApplicationWindow) -> GtkBox {
    let handle = GtkBox::new(Orientation::Horizontal, 8);
    handle.set_margin_top(4);
    handle.set_margin_bottom(4);
    handle.set_margin_start(8);
    handle.set_margin_end(8);
    handle.add_css_class("drag-handle");

    // Drag indicator
    let drag_icon = gtk4::Label::new(Some("⋮⋮"));
    drag_icon.add_css_class("dim-label");
    drag_icon.set_tooltip_text(Some("Drag to move window"));

    let title = gtk4::Label::new(Some("🤫 ShyQuiet"));
    title.add_css_class("title-4");
    title.set_hexpand(true);
    title.set_halign(gtk4::Align::Start);

    handle.append(&drag_icon);
    handle.append(&title);

    // Track drag state
    let start_pos: Rc<Cell<Option<(f64, f64)>>> = Rc::new(Cell::new(None));
    let window_pos: Rc<Cell<(i32, i32)>> = Rc::new(Cell::new((0, 0)));

    // Drag gesture for window movement
    let drag = GestureDrag::new();

    let window_c = window.clone();
    let start_pos_c = start_pos.clone();
    let window_pos_c = window_pos.clone();
    drag.connect_drag_begin(move |_, x, y| {
        start_pos_c.set(Some((x, y)));

        // Get current window position via layer-shell margin
        #[cfg(target_os = "linux")]
        {
            use gtk4_layer_shell::{Edge, LayerShell};
            let margin_left = window_c.margin(Edge::Left);
            let margin_top = window_c.margin(Edge::Top);
            window_pos_c.set((margin_left, margin_top));
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

            // Move window via layer-shell margins
            #[cfg(target_os = "linux")]
            {
                use gtk4_layer_shell::{Edge, LayerShell};
                window_c.set_margin(Edge::Left, new_x);
                window_c.set_margin(Edge::Top, new_y);
            }
        }
    });

    let start_pos_c = start_pos.clone();
    drag.connect_drag_end(move |_, _, _| {
        start_pos_c.set(None);
    });

    handle.add_controller(drag);

    // Visual feedback on hover
    let motion = EventControllerMotion::new();
    let handle_c = handle.clone();
    motion.connect_enter(move |_, _, _| {
        handle_c.add_css_class("drag-handle-hover");
    });
    let handle_c = handle.clone();
    motion.connect_leave(move |_| {
        handle_c.remove_css_class("drag-handle-hover");
    });
    handle.add_controller(motion);

    handle
}

/// Control panel with navigation and action buttons
/// This is the main interactive area that receives input events
fn build_control_panel(stack: &adw::ViewStack, state: Rc<RefCell<AppState>>) -> GtkBox {
    let panel = GtkBox::new(Orientation::Vertical, 4);
    panel.set_margin_top(8);
    panel.set_margin_bottom(8);
    panel.set_margin_start(8);
    panel.set_margin_end(8);
    panel.add_css_class("control-panel");

    // Chat input area (only shown when chat view is active)
    let input_area = views::chat::build_input(state.clone(), stack);
    panel.append(&input_area);

    // Navigation row
    let nav = GtkBox::new(Orientation::Horizontal, 4);
    nav.set_halign(gtk4::Align::Center);
    nav.set_margin_top(4);
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

    // Connect buttons to stack + show/hide input area
    let stack_c = stack.clone();
    let input_c = input_area.clone();
    chat_btn.connect_toggled(move |btn| {
        if btn.is_active() {
            stack_c.set_visible_child_name("chat");
            input_c.set_visible(true);
        }
    });

    let stack_c = stack.clone();
    let input_c = input_area.clone();
    settings_btn.connect_toggled(move |btn| {
        if btn.is_active() {
            stack_c.set_visible_child_name("settings");
            input_c.set_visible(false);
        }
    });

    let stack_c = stack.clone();
    let input_c = input_area.clone();
    history_btn.connect_toggled(move |btn| {
        if btn.is_active() {
            stack_c.set_visible_child_name("history");
            input_c.set_visible(false);
        }
    });

    let stack_c = stack.clone();
    let input_c = input_area.clone();
    prompts_btn.connect_toggled(move |btn| {
        if btn.is_active() {
            stack_c.set_visible_child_name("prompts");
            input_c.set_visible(false);
        }
    });

    nav.append(&chat_btn);
    nav.append(&settings_btn);
    nav.append(&history_btn);
    nav.append(&prompts_btn);

    panel.append(&nav);

    panel
}

#[cfg(target_os = "linux")]
fn init_layer_shell(window: &ApplicationWindow) {
    use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

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

    tracing::info!("Stealth mode enabled (layer-shell: Top, keyboard=None, exclusive=-1)");
}
