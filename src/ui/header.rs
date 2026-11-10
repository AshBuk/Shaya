// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Header bar with navigation and drag handle

use crate::core::state::AppState;
use adw::prelude::*;
use gtk4::gdk;
use gtk4::gio;
use gtk4::{ApplicationWindow, Box as GtkBox, GestureDrag, Label, MenuButton, Orientation};
use libadwaita as adw;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Build header bar: section menu, current section, chat actions, hide
pub fn build(
    window: &ApplicationWindow,
    stack: &adw::ViewStack,
    state: Rc<RefCell<AppState>>,
) -> GtkBox {
    let header = GtkBox::new(Orientation::Horizontal, 2);
    header.add_css_class("shaya-header");

    header.append(&build_navigation(window, stack));

    let brand = Label::new(Some("shaya"));
    brand.add_css_class("shaya-wordmark");
    header.append(&brand);

    let title = Label::new(None);
    title.add_css_class("shaya-title");
    title.set_hexpand(true);
    title.set_xalign(0.0);
    title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    header.append(&title);

    let chat_menu = build_chat_menu();
    header.append(&chat_menu);

    let sync_section = move |stack: &adw::ViewStack| {
        if let Some(child) = stack.visible_child() {
            title.set_text(&stack.page(&child).title().unwrap_or_default());
        }
        chat_menu.set_visible(stack.visible_child_name().as_deref() == Some("chat"));
    };
    sync_section(stack);
    stack.connect_visible_child_name_notify(sync_section);

    let collapse = gtk4::Button::from_icon_name("view-restore-symbolic");
    collapse.add_css_class("flat");
    collapse.set_tooltip_text(Some("Collapse to Shaya orb"));
    collapse.set_action_name(Some("orbit.collapse"));
    header.append(&collapse);
    header.append(&build_hide_button(window));

    // Setup drag gesture for window movement
    setup_drag_gesture(&header, window, state);

    header
}

/// Stack pages in order: the one list of sections the menu and the orbit show.
pub fn sections(stack: &adw::ViewStack) -> Vec<adw::ViewStackPage> {
    stack
        .pages()
        .iter::<adw::ViewStackPage>()
        .filter_map(Result::ok)
        .collect()
}

/// Section menu. The action follows the stack, so every way of switching stays in sync.
fn build_navigation(window: &ApplicationWindow, stack: &adw::ViewStack) -> MenuButton {
    let actions = gio::SimpleActionGroup::new();
    actions.add_action(&gio::PropertyAction::new(
        "section",
        stack,
        "visible-child-name",
    ));
    window.insert_action_group("nav", Some(&actions));

    let menu = gio::Menu::new();
    let settings = gio::Menu::new();
    for page in sections(stack) {
        let name = page.name().unwrap_or_default();
        let target = if name == "settings" { &settings } else { &menu };
        target.append(
            page.title().as_deref(),
            Some(&format!("nav.section::{name}")),
        );
    }
    menu.append_section(None, &settings);

    MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .tooltip_text("Sections")
        .menu_model(&menu)
        .has_frame(false)
        .build()
}

/// Actions on the open chat. Handled by the chat view.
fn build_chat_menu() -> MenuButton {
    let menu = gio::Menu::new();
    menu.append(Some("Save to History"), Some("chat.save"));
    menu.append(Some("Clear Chat"), Some("chat.clear"));

    MenuButton::builder()
        .icon_name("view-more-symbolic")
        .tooltip_text("Chat actions")
        .menu_model(&menu)
        .has_frame(false)
        .build()
}

/// Build hide button
fn build_hide_button(window: &ApplicationWindow) -> gtk4::Button {
    let hide_btn = gtk4::Button::from_icon_name("window-minimize-symbolic");
    hide_btn.set_tooltip_text(Some("Hide"));
    hide_btn.add_css_class("flat");

    let window_c = window.clone();
    hide_btn.connect_clicked(move |_| {
        super::stealth::hide_window(&window_c);
    });

    hide_btn
}

/// Setup drag gesture for window movement (layer-shell)
pub(super) fn setup_drag_gesture(
    widget: &impl IsA<gtk4::Widget>,
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
