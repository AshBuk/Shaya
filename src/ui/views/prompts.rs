// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Prompts View - System prompts manager

use crate::core::state::AppState;
use adw::prelude::*;
use gtk4::{Box as GtkBox, Button, Orientation, PolicyType, ScrolledWindow, TextView, WrapMode};
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
    let active_idx = state_ref.active_prompt_index();

    for (idx, prompt) in state_ref.prompts.iter().enumerate() {
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

        // Active indicator
        let is_active = idx == active_idx;

        let check = gtk4::Image::from_icon_name(if is_active {
            "object-select-symbolic"
        } else {
            "radio-symbolic"
        });
        check.set_margin_end(8);
        row.add_prefix(&check);

        // Activate button (set as active)
        if !is_active {
            let activate_btn = Button::from_icon_name("media-playback-start-symbolic");
            activate_btn.set_valign(gtk4::Align::Center);
            activate_btn.add_css_class("flat");
            activate_btn.set_tooltip_text(Some("Set as active"));

            let state_c = state.clone();
            let container_c = container.clone();
            let prompt_id = prompt.id.clone();

            activate_btn.connect_clicked(move |_| {
                state_c.borrow_mut().active_prompt_id = Some(prompt_id.clone());
                let _ = state_c.borrow().save();
                rebuild_content(&container_c, &state_c);
            });

            row.add_suffix(&activate_btn);
        }

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
        state_c.borrow_mut().active_prompt_id = Some(id);
        let _ = state_c.borrow().save();
        rebuild_content(&container_c, &state_c);
    });

    inner.append(&add_btn);

    // Editor section (for active prompt)
    let editor_header = gtk4::Label::new(Some("Edit Active Prompt"));
    editor_header.set_halign(gtk4::Align::Start);
    editor_header.add_css_class("heading");
    editor_header.set_margin_top(16);
    inner.append(&editor_header);

    // Name entry
    let name_entry = adw::EntryRow::new();
    name_entry.set_title("Name");

    {
        let state_ref = state.borrow();
        let active_idx = state_ref.active_prompt_index();
        if let Some(prompt) = state_ref.prompts.get(active_idx) {
            name_entry.set_text(&prompt.name);
        }
    }

    let name_list = gtk4::ListBox::new();
    name_list.add_css_class("boxed-list");
    name_list.append(&name_entry);
    inner.append(&name_list);

    // Content editor
    let text_view = TextView::new();
    text_view.set_wrap_mode(WrapMode::Word);
    text_view.add_css_class("card");
    text_view.set_top_margin(8);
    text_view.set_bottom_margin(8);
    text_view.set_left_margin(8);
    text_view.set_right_margin(8);

    {
        let state_ref = state.borrow();
        let active_idx = state_ref.active_prompt_index();
        if let Some(prompt) = state_ref.prompts.get(active_idx) {
            text_view.buffer().set_text(&prompt.content);
        }
    }

    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .vexpand(true)
        .min_content_height(150)
        .child(&text_view)
        .build();
    inner.append(&scroll);

    // Save button
    let btn_box = GtkBox::new(Orientation::Horizontal, 8);
    btn_box.set_halign(gtk4::Align::End);
    btn_box.set_margin_top(8);

    let save_btn = Button::with_label("Save");
    save_btn.add_css_class("suggested-action");

    let state_c = state.clone();
    let container_c = container.clone();

    save_btn.connect_clicked(move |_| {
        let name = name_entry.text().to_string();
        let buffer = text_view.buffer();
        let (start, end) = buffer.bounds();
        let content = buffer.text(&start, &end, false).to_string();

        let mut state_mut = state_c.borrow_mut();
        let active_idx = state_mut.active_prompt_index();

        if let Some(prompt) = state_mut.prompts.get_mut(active_idx) {
            prompt.name = name;
            prompt.content = content;
        }

        let _ = state_mut.save();
        drop(state_mut);
        rebuild_content(&container_c, &state_c);
    });

    btn_box.append(&save_btn);
    inner.append(&btn_box);

    container.append(&inner);
}
