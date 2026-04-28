// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Header bar with navigation and drag handle

use crate::core::state::AppState;
use adw::prelude::*;
use gtk4::gdk;
use gtk4::{ApplicationWindow, Box as GtkBox, GestureDrag, Orientation, ToggleButton};
use libadwaita as adw;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Build header bar with navigation and drag handle
pub fn build(
    window: &ApplicationWindow,
    stack: &adw::ViewStack,
    state: Rc<RefCell<AppState>>,
) -> GtkBox {
    let header = GtkBox::new(Orientation::Horizontal, 4);
    header.set_margin_top(4);
    header.set_margin_bottom(4);
    header.set_margin_start(8);
    header.set_margin_end(8);
    header.add_css_class("header-bar");

    // Drag indicator (left)
    let drag_icon = gtk4::Label::new(Some("⋮⋮"));
    drag_icon.add_css_class("dim-label");
    drag_icon.set_tooltip_text(Some("Drag to move"));
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

    // Setup drag gesture for window movement
    setup_drag_gesture(&header, window, state);

    header
}

/// Build navigation buttons (💬 📋 📝 📜 ⚙️ ⏺)
fn build_navigation(stack: &adw::ViewStack) -> GtkBox {
    let nav = GtkBox::new(Orientation::Horizontal, 2);
    nav.add_css_class("linked");

    let chat_btn = ToggleButton::with_label("💬");
    chat_btn.set_tooltip_text(Some("Chat"));
    chat_btn.set_active(true);

    let cheatsheets_btn = ToggleButton::with_label("📋");
    cheatsheets_btn.set_tooltip_text(Some("Cheatsheets"));
    cheatsheets_btn.set_group(Some(&chat_btn));

    let prompts_btn = ToggleButton::with_label("📝");
    prompts_btn.set_tooltip_text(Some("Prompts"));
    prompts_btn.set_group(Some(&chat_btn));

    let history_btn = ToggleButton::with_label("📜");
    history_btn.set_tooltip_text(Some("History"));
    history_btn.set_group(Some(&chat_btn));

    let settings_btn = ToggleButton::with_label("⚙️");
    settings_btn.set_tooltip_text(Some("Settings"));
    settings_btn.set_group(Some(&chat_btn));

    let meetings_btn = ToggleButton::with_label("⏺");
    meetings_btn.set_tooltip_text(Some("Meetings"));
    meetings_btn.set_group(Some(&chat_btn));

    // Connect to stack
    let stack_c = stack.clone();
    chat_btn.connect_toggled(move |btn| {
        if btn.is_active() {
            stack_c.set_visible_child_name("chat");
        }
    });

    let stack_c = stack.clone();
    cheatsheets_btn.connect_toggled(move |btn| {
        if btn.is_active() {
            stack_c.set_visible_child_name("cheatsheets");
        }
    });

    let stack_c = stack.clone();
    prompts_btn.connect_toggled(move |btn| {
        if btn.is_active() {
            stack_c.set_visible_child_name("prompts");
        }
    });

    let stack_c = stack.clone();
    history_btn.connect_toggled(move |btn| {
        if btn.is_active() {
            stack_c.set_visible_child_name("history");
        }
    });

    let stack_c = stack.clone();
    settings_btn.connect_toggled(move |btn| {
        if btn.is_active() {
            stack_c.set_visible_child_name("settings");
        }
    });

    let stack_c = stack.clone();
    meetings_btn.connect_toggled(move |btn| {
        if btn.is_active() {
            stack_c.set_visible_child_name("meetings");
        }
    });

    nav.append(&chat_btn);
    nav.append(&cheatsheets_btn);
    nav.append(&prompts_btn);
    nav.append(&history_btn);
    nav.append(&settings_btn);
    nav.append(&meetings_btn);

    nav
}

/// Build hide button
fn build_hide_button(window: &ApplicationWindow) -> gtk4::Button {
    let hide_btn = gtk4::Button::with_label("—");
    hide_btn.set_tooltip_text(Some("Hide"));
    hide_btn.add_css_class("flat");

    let window_c = window.clone();
    hide_btn.connect_clicked(move |_| {
        super::stealth::hide_window(&window_c);
    });

    hide_btn
}

/// Setup drag gesture for window movement (layer-shell)
pub fn setup_drag_gesture(
    widget: &GtkBox,
    window: &ApplicationWindow,
    state: Rc<RefCell<AppState>>,
) {
    let window_pos: Rc<Cell<(i32, i32)>> = Rc::new(Cell::new((0, 0)));

    let drag = GestureDrag::new();

    // Drag begin: capture current margins
    let window_c = window.clone();
    let window_pos_c = window_pos.clone();
    drag.connect_drag_begin(move |_, _, _| {
        #[cfg(target_os = "linux")]
        {
            use gtk4_layer_shell::{Edge, LayerShell};
            if window_c.is_layer_window() {
                window_pos_c.set((window_c.margin(Edge::Left), window_c.margin(Edge::Top)));
            }
        }
    });

    // Drag update: apply offset (layer-shell) or delegate to WM on first update (fallback)
    let window_c = window.clone();
    let window_pos_c = window_pos.clone();
    let wm_move_started = Rc::new(Cell::new(false));
    let wm_move_started_c = wm_move_started.clone();
    drag.connect_drag_update(move |gesture, offset_x, offset_y| {
        #[cfg(target_os = "linux")]
        {
            use gtk4_layer_shell::{Edge, LayerShell};
            if window_c.is_layer_window() {
                let (start_x, start_y) = window_pos_c.get();
                window_c.set_margin(Edge::Left, start_x + offset_x as i32);
                window_c.set_margin(Edge::Top, start_y + offset_y as i32);
                return;
            }
        }
        // Fallback: delegate to WM once (GNOME, etc.)
        if !wm_move_started_c.get() {
            wm_move_started_c.set(true);
            if let Some(surface) = window_c.surface() {
                if let Some(toplevel) = surface.dynamic_cast_ref::<gdk::Toplevel>() {
                    if let Some(device) = gesture.device() {
                        let (start_x, start_y) = gesture.start_point().unwrap_or((0.0, 0.0));
                        toplevel.begin_move(
                            &device,
                            gesture.current_button() as i32,
                            start_x + offset_x,
                            start_y + offset_y,
                            gesture.current_event_time(),
                        );
                    }
                }
            }
        }
    });

    // Drag end: reset WM flag + save position (layer-shell)
    let wm_move_started_c = wm_move_started;
    let window_c = window.clone();
    drag.connect_drag_end(move |_, _, _| {
        wm_move_started_c.set(false);
        #[cfg(target_os = "linux")]
        {
            use gtk4_layer_shell::{Edge, LayerShell};
            if window_c.is_layer_window() {
                let x = window_c.margin(Edge::Left);
                let y = window_c.margin(Edge::Top);
                let mut st = state.borrow_mut();
                st.settings.window_x = x;
                st.settings.window_y = y;
                if let Err(e) = st.save() {
                    tracing::warn!("Failed to save window position: {}", e);
                }
            }
        }
    });

    widget.add_controller(drag);
}
