// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! History View - Past conversations

use crate::core::state::AppState;
use adw::prelude::*;
use gtk4::Box as GtkBox;
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;

/// Build the history view
pub fn build(state: Rc<RefCell<AppState>>) -> GtkBox {
    let vbox = GtkBox::new(gtk4::Orientation::Vertical, 0);

    let state_ref = state.borrow();

    if state_ref.history.is_empty() {
        // Empty state
        let status = adw::StatusPage::new();
        status.set_icon_name(Some("chat-symbolic"));
        status.set_title("No Conversations Yet");
        status.set_description(Some("Start a chat to see history here"));
        status.set_vexpand(true);
        vbox.append(&status);
    } else {
        // List of conversations
        let list = gtk4::ListBox::new();
        list.set_selection_mode(gtk4::SelectionMode::None);
        list.add_css_class("boxed-list");
        list.set_margin_top(12);
        list.set_margin_bottom(12);
        list.set_margin_start(12);
        list.set_margin_end(12);

        for conv in &state_ref.history {
            let row = adw::ActionRow::new();
            row.set_title(&conv.title);
            row.set_subtitle(&conv.created_at);
            row.add_suffix(&gtk4::Image::from_icon_name("go-next-symbolic"));
            row.set_activatable(true);
            list.append(&row);
        }

        let scroll = gtk4::ScrolledWindow::builder()
            .vexpand(true)
            .child(&list)
            .build();

        vbox.append(&scroll);
    }

    vbox
}
