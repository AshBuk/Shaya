// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Prompts View - System prompts editor

use crate::core::state::AppState;
use adw::prelude::*;
use gtk4::{Box as GtkBox, Orientation, PolicyType, ScrolledWindow, TextView, WrapMode};
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;

/// Build the prompts view
pub fn build(state: Rc<RefCell<AppState>>) -> GtkBox {
    let vbox = GtkBox::new(Orientation::Vertical, 12);
    vbox.set_margin_top(12);
    vbox.set_margin_bottom(12);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);

    // Header
    let header = adw::PreferencesGroup::new();
    header.set_title("System Prompts");
    header.set_description(Some("Customize AI behavior with system prompts"));
    vbox.append(&header);

    // Prompt selector
    let prompt_row = adw::ComboRow::new();
    prompt_row.set_title("Active Prompt");

    let state_ref = state.borrow();
    let prompt_names: Vec<&str> = state_ref.prompts.iter().map(|p| p.name.as_str()).collect();
    let prompts_list = gtk4::StringList::new(&prompt_names);
    prompt_row.set_model(Some(&prompts_list));
    drop(state_ref);

    vbox.append(&prompt_row);

    // Prompt editor
    let editor_label = gtk4::Label::new(Some("Edit Prompt"));
    editor_label.set_halign(gtk4::Align::Start);
    editor_label.add_css_class("heading");
    editor_label.set_margin_top(12);
    vbox.append(&editor_label);

    let text_view = TextView::new();
    text_view.set_wrap_mode(WrapMode::Word);
    text_view.add_css_class("card");
    text_view.set_top_margin(8);
    text_view.set_bottom_margin(8);
    text_view.set_left_margin(8);
    text_view.set_right_margin(8);

    // Set current prompt content
    if let Some(prompt) = state.borrow().prompts.first() {
        text_view.buffer().set_text(&prompt.content);
    }

    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .vexpand(true)
        .min_content_height(200)
        .child(&text_view)
        .build();

    vbox.append(&scroll);

    // Buttons
    let btn_box = GtkBox::new(Orientation::Horizontal, 8);
    btn_box.set_halign(gtk4::Align::End);
    btn_box.set_margin_top(8);

    let new_btn = gtk4::Button::with_label("New Prompt");

    let save_btn = gtk4::Button::with_label("Save");
    save_btn.add_css_class("suggested-action");

    let state_clone = state.clone();
    let text_view_clone = text_view.clone();
    save_btn.connect_clicked(move |_| {
        let buffer = text_view_clone.buffer();
        let (start, end) = buffer.bounds();
        let content = buffer.text(&start, &end, false).to_string();

        if let Some(prompt) = state_clone.borrow_mut().prompts.first_mut() {
            prompt.content = content;
        }

        if let Err(e) = state_clone.borrow().save() {
            tracing::error!("Failed to save prompts: {}", e);
        }
    });

    btn_box.append(&new_btn);
    btn_box.append(&save_btn);
    vbox.append(&btn_box);

    vbox
}
