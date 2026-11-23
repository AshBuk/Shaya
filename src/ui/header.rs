// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Header bar with navigation and drag handle

use crate::core::state::AppState;
use adw::prelude::*;
use gtk4::gio;
use gtk4::{
    ApplicationWindow, Box as GtkBox, CenterBox, GestureDrag, Label, MenuButton, Orientation,
};
use libadwaita as adw;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Build header bar: section menu and current section, the app name in the
/// centre, chat actions and window buttons at the end.
///
/// The overlay moves itself and has collapse and hide. A normal window uses
/// it as the titlebar: the window manager moves it and GTK draws the buttons.
pub fn build(
    window: &ApplicationWindow,
    stack: &adw::ViewStack,
    state: Rc<RefCell<AppState>>,
    overlay: bool,
) -> CenterBox {
    let header = CenterBox::new();
    header.add_css_class("shaya-header");

    let start = GtkBox::new(Orientation::Horizontal, 2);
    start.append(&build_navigation(window, stack));
    let title = Label::new(None);
    title.add_css_class("shaya-title");
    title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    start.append(&title);
    header.set_start_widget(Some(&start));

    let brand = Label::new(Some("Shaya"));
    brand.add_css_class("shaya-wordmark");
    header.set_center_widget(Some(&brand));

    let end = GtkBox::new(Orientation::Horizontal, 2);
    let chat_menu = build_chat_menu();
    end.append(&chat_menu);

    let sync_section = move |stack: &adw::ViewStack| {
        if let Some(child) = stack.visible_child() {
            title.set_text(&stack.page(&child).title().unwrap_or_default());
        }
        chat_menu.set_visible(stack.visible_child_name().as_deref() == Some("chat"));
    };
    sync_section(stack);
    stack.connect_visible_child_name_notify(sync_section);

    if overlay {
        let collapse = gtk4::Button::from_icon_name("view-restore-symbolic");
        collapse.add_css_class("flat");
        collapse.set_tooltip_text(Some("Collapse to Shaya orb"));
        collapse.set_action_name(Some("orbit.collapse"));
        end.append(&collapse);
        end.append(&build_hide_button());
        setup_drag_gesture(&header, window, state);
    } else {
        end.append(&gtk4::WindowControls::new(gtk4::PackType::End));
    }
    header.set_end_widget(Some(&end));

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
    let app_section = gio::Menu::new();
    app_section.append(Some("Quit"), Some("win.quit"));
    menu.append_section(None, &app_section);

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

/// Hide button. Running `shaya` again brings the window back.
fn build_hide_button() -> gtk4::Button {
    let hide_btn = gtk4::Button::from_icon_name("window-minimize-symbolic");
    hide_btn.set_tooltip_text(Some("Hide"));
    hide_btn.add_css_class("flat");
    hide_btn.set_action_name(Some("win.hide"));
    hide_btn
}

/// Drag to move the overlay. Layer-shell places it by its margins.
pub(super) fn setup_drag_gesture(
    widget: &impl IsA<gtk4::Widget>,
    window: &ApplicationWindow,
    state: Rc<RefCell<AppState>>,
) {
    #[cfg(target_os = "linux")]
    {
        use gtk4_layer_shell::{Edge, LayerShell};

        let start = Rc::new(Cell::new((0, 0)));
        let drag = GestureDrag::new();

        let (window_c, start_c) = (window.clone(), start.clone());
        drag.connect_drag_begin(move |_, _, _| {
            start_c.set((window_c.margin(Edge::Left), window_c.margin(Edge::Top)));
        });

        let window_c = window.clone();
        drag.connect_drag_update(move |_, offset_x, offset_y| {
            let (x, y) = start.get();
            super::layer_shell::place(&window_c, x + offset_x as i32, y + offset_y as i32);
        });

        let window_c = window.clone();
        drag.connect_drag_end(move |_, _, _| {
            let mut st = state.borrow_mut();
            st.settings.window_x = window_c.margin(Edge::Left);
            st.settings.window_y = window_c.margin(Edge::Top);
            if let Err(e) = st.save() {
                tracing::warn!("Failed to save window position: {}", e);
            }
        });

        widget.add_controller(drag);
    }
    #[cfg(not(target_os = "linux"))]
    let _ = (widget, window, state);
}
