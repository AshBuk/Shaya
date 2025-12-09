// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! History View - Past conversations

use super::chat;
use crate::core::state::AppState;
use adw::prelude::*;
use gtk4::{Box as GtkBox, Button};
use libadwaita as adw;
use libadwaita::prelude::MessageDialogExt;
use std::cell::RefCell;
use std::rc::Rc;

/// Build the history view
pub fn build(state: Rc<RefCell<AppState>>, stack: &adw::ViewStack) -> GtkBox {
    let vbox = GtkBox::new(gtk4::Orientation::Vertical, 0);
    vbox.set_vexpand(true);

    // Content container (will be rebuilt on refresh)
    let content = GtkBox::new(gtk4::Orientation::Vertical, 0);
    content.set_vexpand(true);
    vbox.append(&content);

    rebuild_content(&content, &state, stack);

    // Auto-refresh when view becomes visible
    let content_c = content.clone();
    let state_c = state.clone();
    let stack_c = stack.clone();
    stack.connect_visible_child_name_notify(move |s| {
        if s.visible_child_name().as_deref() == Some("history") {
            rebuild_content(&content_c, &state_c, &stack_c);
        }
    });

    vbox
}

/// Rebuild history content (called on refresh)
fn rebuild_content(container: &GtkBox, state: &Rc<RefCell<AppState>>, stack: &adw::ViewStack) {
    // Clear existing children
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }

    let state_ref = state.borrow();

    if state_ref.history.is_empty() {
        // Empty state
        let status = adw::StatusPage::new();
        status.set_icon_name(Some("chat-symbolic"));
        status.set_title("No Conversations Yet");
        status.set_description(Some("Start a chat and save it to see history here"));
        status.set_vexpand(true);
        container.append(&status);
    } else {
        // List of conversations (newest first)
        let list = gtk4::ListBox::new();
        list.set_selection_mode(gtk4::SelectionMode::None);
        list.add_css_class("boxed-list");
        list.set_margin_top(12);
        list.set_margin_bottom(12);
        list.set_margin_start(12);
        list.set_margin_end(12);

        // Iterate in reverse (newest first)
        for (index, conv) in state_ref.history.iter().enumerate().rev() {
            let row = adw::ActionRow::new();
            row.set_title(&conv.title);
            row.set_subtitle(&format!(
                "{} • {} messages",
                conv.created_at,
                conv.messages.len()
            ));

            // Delete button
            let delete_btn = Button::from_icon_name("user-trash-symbolic");
            delete_btn.set_valign(gtk4::Align::Center);
            delete_btn.add_css_class("flat");
            delete_btn.set_tooltip_text(Some("Delete conversation"));

            {
                let state_c = state.clone();
                let container_c = container.clone();
                let stack_c = stack.clone();

                delete_btn.connect_clicked(move |btn| {
                    let dialog = adw::MessageDialog::new(
                        btn.root()
                            .and_then(|r| r.downcast::<gtk4::Window>().ok())
                            .as_ref(),
                        Some("Delete conversation?"),
                        Some("This cannot be undone."),
                    );
                    dialog.add_response("cancel", "Cancel");
                    dialog.add_response("delete", "Delete");
                    dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
                    dialog.set_default_response(Some("cancel"));

                    let state_cc = state_c.clone();
                    let container_cc = container_c.clone();
                    let stack_cc = stack_c.clone();

                    dialog.connect_response(None, move |d, response| {
                        if response == "delete" {
                            let mut state_mut = state_cc.borrow_mut();
                            if index < state_mut.history.len() {
                                state_mut.history.remove(index);
                                if let Err(e) = state_mut.save() {
                                    tracing::error!("Failed to save after delete: {}", e);
                                }
                            }
                            drop(state_mut);
                            rebuild_content(&container_cc, &state_cc, &stack_cc);
                        }
                        d.close();
                    });

                    dialog.present();
                });
            }

            row.add_suffix(&delete_btn);
            row.add_suffix(&gtk4::Image::from_icon_name("go-next-symbolic"));
            row.set_activatable(true);

            // Load conversation on click
            {
                let state_c = state.clone();
                let stack_c = stack.clone();
                let messages = conv.messages.clone();

                row.connect_activated(move |_| {
                    // Copy messages to current chat
                    state_c.borrow_mut().current_messages = messages.clone();

                    // Switch to chat view
                    stack_c.set_visible_child_name("chat");

                    // Refresh chat display
                    chat::refresh_display(&state_c.borrow());
                });
            }

            list.append(&row);
        }

        let scroll = gtk4::ScrolledWindow::builder()
            .vexpand(true)
            .child(&list)
            .build();

        container.append(&scroll);
    }
}
