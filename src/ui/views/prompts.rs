// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Prompts View - System prompts manager

use crate::core::state::AppState;
use adw::prelude::*;
use gtk4::{Box as GtkBox, Button, Orientation, PolicyType, ScrolledWindow};
use libadwaita as adw;
use libadwaita::prelude::MessageDialogExt;
use std::cell::RefCell;
use std::rc::Rc;

/// Build the prompts view
pub fn build(state: Rc<RefCell<AppState>>) -> GtkBox {
    let vbox = GtkBox::new(Orientation::Vertical, 0);
    vbox.set_vexpand(true);

    // Content container (rebuilt on changes)
    let content = GtkBox::new(Orientation::Vertical, 0);
    content.set_vexpand(true);
    vbox.append(&content);

    rebuild_content(&content, &state);

    vbox
}

fn rebuild_content(container: &GtkBox, state: &Rc<RefCell<AppState>>) {
    // Clear existing
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }

    let inner = GtkBox::new(Orientation::Vertical, 12);
    inner.set_margin_top(12);
    inner.set_margin_bottom(12);
    inner.set_margin_start(12);
    inner.set_margin_end(12);

    // Header
    let header = adw::PreferencesGroup::new();
    header.set_title("System Prompts");
    header.set_description(Some("Manage prompts for different scenarios"));
    inner.append(&header);

    // Prompts list
    let list = gtk4::ListBox::new();
    list.set_selection_mode(gtk4::SelectionMode::None);
    list.add_css_class("boxed-list");

    let state_ref = state.borrow();

    for prompt in state_ref.prompts.iter() {
        let row = adw::ActionRow::new();
        row.set_title(&prompt.name);

        // Truncate content for subtitle
        let preview: String = prompt.content.chars().take(50).collect();
        let preview = if prompt.content.len() > 50 {
            format!("{}...", preview)
        } else {
            preview
        };
        row.set_subtitle(&preview);

        // Checkbox for multi-select
        let is_active = state_ref.is_prompt_active(&prompt.id);
        let check_btn = gtk4::CheckButton::new();
        check_btn.set_active(is_active);
        check_btn.set_valign(gtk4::Align::Center);

        let state_c = state.clone();
        let container_c = container.clone();
        let prompt_id = prompt.id.clone();

        check_btn.connect_toggled(move |_| {
            state_c.borrow_mut().toggle_prompt(&prompt_id);
            let _ = state_c.borrow().save();
            rebuild_content(&container_c, &state_c);
        });

        row.add_prefix(&check_btn);

        // Edit button
        let edit_btn = Button::from_icon_name("document-edit-symbolic");
        edit_btn.set_valign(gtk4::Align::Center);
        edit_btn.add_css_class("flat");
        edit_btn.set_tooltip_text(Some("Edit prompt"));

        let state_c = state.clone();
        let container_c = container.clone();
        let prompt_id = prompt.id.clone();
        let prompt_name = prompt.name.clone();
        let prompt_content = prompt.content.clone();

        edit_btn.connect_clicked(move |btn| {
            show_edit_dialog(
                btn,
                &state_c,
                &container_c,
                &prompt_id,
                &prompt_name,
                &prompt_content,
            );
        });

        row.add_suffix(&edit_btn);

        // Delete button (if more than one prompt)
        if state_ref.prompts.len() > 1 {
            let delete_btn = Button::from_icon_name("user-trash-symbolic");
            delete_btn.set_valign(gtk4::Align::Center);
            delete_btn.add_css_class("flat");
            delete_btn.set_tooltip_text(Some("Delete prompt"));

            let state_c = state.clone();
            let container_c = container.clone();
            let prompt_id = prompt.id.clone();

            delete_btn.connect_clicked(move |btn| {
                let dialog = adw::MessageDialog::new(
                    btn.root()
                        .and_then(|r| r.downcast::<gtk4::Window>().ok())
                        .as_ref(),
                    Some("Delete prompt?"),
                    Some("This cannot be undone."),
                );
                dialog.add_response("cancel", "Cancel");
                dialog.add_response("delete", "Delete");
                dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);

                let state_cc = state_c.clone();
                let container_cc = container_c.clone();
                let id = prompt_id.clone();

                dialog.connect_response(None, move |d, response| {
                    if response == "delete" {
                        state_cc.borrow_mut().remove_prompt(&id);
                        let _ = state_cc.borrow().save();
                        rebuild_content(&container_cc, &state_cc);
                    }
                    d.close();
                });

                dialog.present();
            });

            row.add_suffix(&delete_btn);
        }

        list.append(&row);
    }
    drop(state_ref);

    inner.append(&list);

    // Add new prompt button
    let add_btn = Button::with_label("+ Add New Prompt");
    add_btn.add_css_class("suggested-action");
    add_btn.set_margin_top(8);

    let state_c = state.clone();
    let container_c = container.clone();

    add_btn.connect_clicked(move |_| {
        let id = state_c.borrow_mut().add_prompt(
            "New Prompt".to_string(),
            "You are a helpful AI assistant.".to_string(),
        );
        // Auto-activate new prompt
        state_c.borrow_mut().active_prompt_ids.push(id);
        let _ = state_c.borrow().save();
        rebuild_content(&container_c, &state_c);
    });

    inner.append(&add_btn);

    // Wrap in ScrolledWindow to allow vertical resize
    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .vexpand(true)
        .child(&inner)
        .build();

    container.append(&scroll);
}

/// Show edit dialog for a prompt
fn show_edit_dialog(
    btn: &Button,
    state: &Rc<RefCell<AppState>>,
    container: &GtkBox,
    prompt_id: &str,
    name: &str,
    content: &str,
) {
    let dialog = adw::Dialog::new();
    dialog.set_title("Edit Prompt");
    dialog.set_content_width(400);
    dialog.set_content_height(300);

    let toolbar = adw::ToolbarView::new();

    // Header bar with save button
    let header = adw::HeaderBar::new();
    let save_btn = Button::with_label("Save");
    save_btn.add_css_class("suggested-action");
    header.pack_end(&save_btn);
    toolbar.add_top_bar(&header);

    // Content
    let vbox = GtkBox::new(Orientation::Vertical, 12);
    vbox.set_margin_top(12);
    vbox.set_margin_bottom(12);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);

    // Name entry
    let name_entry = adw::EntryRow::new();
    name_entry.set_title("Name");
    name_entry.set_text(name);

    let name_list = gtk4::ListBox::new();
    name_list.add_css_class("boxed-list");
    name_list.append(&name_entry);
    vbox.append(&name_list);

    // Content editor
    let text_view = gtk4::TextView::new();
    text_view.set_wrap_mode(gtk4::WrapMode::Word);
    text_view.add_css_class("card");
    text_view.set_top_margin(8);
    text_view.set_bottom_margin(8);
    text_view.set_left_margin(8);
    text_view.set_right_margin(8);
    text_view.buffer().set_text(content);

    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .vexpand(true)
        .min_content_height(100)
        .child(&text_view)
        .build();
    vbox.append(&scroll);

    toolbar.set_content(Some(&vbox));
    dialog.set_child(Some(&toolbar));

    // Save handler
    let state_c = state.clone();
    let container_c = container.clone();
    let dialog_c = dialog.clone();
    let id = prompt_id.to_string();

    save_btn.connect_clicked(move |_| {
        let new_name = name_entry.text().to_string();
        let buffer = text_view.buffer();
        let (start, end) = buffer.bounds();
        let new_content = buffer.text(&start, &end, false).to_string();

        {
            let mut state_mut = state_c.borrow_mut();
            if let Some(prompt) = state_mut.prompts.iter_mut().find(|p| p.id == id) {
                prompt.name = new_name;
                prompt.content = new_content;
            }
            let _ = state_mut.save();
        }

        dialog_c.close();
        rebuild_content(&container_c, &state_c);
    });

    if let Some(window) = btn.root().and_then(|r| r.downcast::<gtk4::Window>().ok()) {
        dialog.present(Some(&window));
    }
}
