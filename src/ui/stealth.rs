// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Stealth Window - GTK4 + Layer-Shell
//!
//! Unified invisible overlay with ViewStack for all functionality.

use crate::core::state::AppState;
use crate::ui::views;
use adw::prelude::*;
use gtk4::{glib, Application, ApplicationWindow, Box as GtkBox, Orientation, ToggleButton};
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;

const APP_ID: &str = "com.shyquiet.stealth";

/// Run the stealth window application
pub fn run() -> anyhow::Result<()> {
    let app = Application::builder().application_id(APP_ID).build();

    app.connect_activate(|app| {
        // Load state from disk
        let state = Rc::new(RefCell::new(AppState::load()));
        build_ui(app, state);
    });

    let exit_code = app.run();
    if exit_code == glib::ExitCode::SUCCESS {
        Ok(())
    } else {
        anyhow::bail!("Application exited with error")
    }
}

fn build_ui(app: &Application, state: Rc<RefCell<AppState>>) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("ShyQuiet")
        .default_width(400)
        .default_height(550)
        .build();

    // Initialize layer-shell for stealth mode
    #[cfg(target_os = "linux")]
    init_layer_shell(&window);

    // Main container
    let main_box = GtkBox::new(Orientation::Vertical, 0);

    // Header with navigation
    let header = build_header();
    main_box.append(&header);

    // ViewStack for different views
    let stack = adw::ViewStack::new();
    stack.set_vexpand(true);

    // Chat view
    let chat_view = views::chat::build(state.clone());
    stack.add_titled(&chat_view, Some("chat"), "Chat");
    stack.page(&chat_view).set_icon_name(Some("chat-symbolic"));

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

    // Navigation buttons
    let nav_box = build_nav_buttons(&stack);
    main_box.append(&nav_box);

    main_box.append(&stack);

    window.set_child(Some(&main_box));
    window.present();
}

fn build_header() -> GtkBox {
    let header = GtkBox::new(Orientation::Horizontal, 8);
    header.set_margin_top(8);
    header.set_margin_bottom(4);
    header.set_margin_start(12);
    header.set_margin_end(12);

    let title = gtk4::Label::new(Some("🤫 ShyQuiet"));
    title.add_css_class("title-3");

    let spacer = GtkBox::new(Orientation::Horizontal, 0);
    spacer.set_hexpand(true);

    header.append(&title);
    header.append(&spacer);

    header
}

fn build_nav_buttons(stack: &adw::ViewStack) -> GtkBox {
    let nav = GtkBox::new(Orientation::Horizontal, 4);
    nav.set_halign(gtk4::Align::Center);
    nav.set_margin_top(4);
    nav.set_margin_bottom(8);
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

    // Connect buttons to stack
    let stack_clone = stack.clone();
    chat_btn.connect_toggled(move |btn| {
        if btn.is_active() {
            stack_clone.set_visible_child_name("chat");
        }
    });

    let stack_clone = stack.clone();
    settings_btn.connect_toggled(move |btn| {
        if btn.is_active() {
            stack_clone.set_visible_child_name("settings");
        }
    });

    let stack_clone = stack.clone();
    history_btn.connect_toggled(move |btn| {
        if btn.is_active() {
            stack_clone.set_visible_child_name("history");
        }
    });

    let stack_clone = stack.clone();
    prompts_btn.connect_toggled(move |btn| {
        if btn.is_active() {
            stack_clone.set_visible_child_name("prompts");
        }
    });

    nav.append(&chat_btn);
    nav.append(&settings_btn);
    nav.append(&history_btn);
    nav.append(&prompts_btn);

    nav
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
    window.set_layer(Layer::Overlay);
    window.set_keyboard_mode(KeyboardMode::OnDemand);

    // Floating window (no anchors)
    window.set_anchor(Edge::Left, false);
    window.set_anchor(Edge::Right, false);
    window.set_anchor(Edge::Top, false);
    window.set_anchor(Edge::Bottom, false);

    window.set_namespace("shyquiet-stealth");

    tracing::info!("Stealth mode enabled (layer-shell)");
}
