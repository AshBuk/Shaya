// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Header bar with navigation and drag handle

use adw::prelude::*;
use gtk4::{
    ApplicationWindow, Box as GtkBox, EventControllerMotion, GestureDrag, Orientation,
    ToggleButton,
};
use libadwaita as adw;
use std::cell::Cell;
use std::rc::Rc;

/// Build header bar with navigation and drag handle
pub fn build(window: &ApplicationWindow, stack: &adw::ViewStack) -> GtkBox {
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

    // Spacer left
    let spacer_left = GtkBox::new(Orientation::Horizontal, 0);
    spacer_left.set_hexpand(true);
    header.append(&spacer_left);

    // Navigation buttons
    let nav = build_navigation(stack);
    header.append(&nav);

    // Spacer right
    let spacer_right = GtkBox::new(Orientation::Horizontal, 0);
    spacer_right.set_hexpand(true);
    header.append(&spacer_right);

    // Hide button
    let hide_btn = build_hide_button(window);
    header.append(&hide_btn);

    // Drag indicator (right)
    let drag_icon_r = gtk4::Label::new(Some("⋮⋮"));
    drag_icon_r.add_css_class("dim-label");
    header.append(&drag_icon_r);

    // Setup drag gesture for window movement
    setup_drag_gesture(&header, window);

    // Setup hover feedback
    setup_hover_feedback(&header);

    header
}

/// Build navigation buttons (💬 ⚙️ 📜 📝)
fn build_navigation(stack: &adw::ViewStack) -> GtkBox {
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

    // Connect to stack
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

    nav
}

/// Build hide button
fn build_hide_button(window: &ApplicationWindow) -> gtk4::Button {
    let hide_btn = gtk4::Button::with_label("—");
    hide_btn.set_tooltip_text(Some("Hide (Ctrl+Alt+S or ./dev run to show)"));
    hide_btn.add_css_class("flat");

    let window_c = window.clone();
    hide_btn.connect_clicked(move |_| {
        tracing::debug!("Hide button clicked");
        window_c.set_visible(false);
    });

    hide_btn
}

/// Setup drag gesture for window movement (layer-shell)
fn setup_drag_gesture(header: &GtkBox, window: &ApplicationWindow) {
    let start_pos: Rc<Cell<Option<(f64, f64)>>> = Rc::new(Cell::new(None));
    let window_pos: Rc<Cell<(i32, i32)>> = Rc::new(Cell::new((0, 0)));

    let drag = GestureDrag::new();

    // Drag begin
    let window_c = window.clone();
    let start_pos_c = start_pos.clone();
    let window_pos_c = window_pos.clone();
    drag.connect_drag_begin(move |_, x, y| {
        start_pos_c.set(Some((x, y)));

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

    // Drag update
    let window_c = window.clone();
    let start_pos_c = start_pos.clone();
    let window_pos_c = window_pos.clone();
    drag.connect_drag_update(move |_, offset_x, offset_y| {
        if start_pos_c.get().is_some() {
            let (start_x, start_y) = window_pos_c.get();
            let new_x = start_x + offset_x as i32;
            let new_y = start_y + offset_y as i32;

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

    // Drag end
    let start_pos_c = start_pos.clone();
    drag.connect_drag_end(move |_, _, _| {
        start_pos_c.set(None);
    });

    header.add_controller(drag);
}

/// Setup hover feedback on header
fn setup_hover_feedback(header: &GtkBox) {
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
}

