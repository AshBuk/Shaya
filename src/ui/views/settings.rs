// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Settings View - API keys and configuration

use crate::core::models::MODELS;
use crate::core::state::AppState;
use adw::prelude::*;
use gtk4::Box as GtkBox;
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;

/// Build the settings view
pub fn build(state: Rc<RefCell<AppState>>) -> GtkBox {
    let vbox = GtkBox::new(gtk4::Orientation::Vertical, 0);

    let page = adw::PreferencesPage::new();

    // AI Provider group
    let ai_group = adw::PreferencesGroup::new();
    ai_group.set_title("AI Provider");
    ai_group.set_description(Some("Configure your AI provider (Groq)"));

    // API Key
    let api_key_row = adw::PasswordEntryRow::new();
    api_key_row.set_title("API Key");
    api_key_row.set_text(&state.borrow().settings.api_key);

    let state_clone = state.clone();
    api_key_row.connect_changed(move |row| {
        state_clone.borrow_mut().settings.api_key = row.text().to_string();
    });

    ai_group.add(&api_key_row);

    // Model selection (using MODELS constant)
    let model_row = adw::ComboRow::new();
    model_row.set_title("Model");
    model_row.set_subtitle("Vision models marked with 👁️");

    // Build display names with Vision indicator
    let model_names: Vec<String> = MODELS
        .iter()
        .map(|(_, name, vision)| {
            if *vision {
                format!("👁️ {}", name)
            } else {
                name.to_string()
            }
        })
        .collect();
    let model_names_refs: Vec<&str> = model_names.iter().map(|s| s.as_str()).collect();
    let models = gtk4::StringList::new(&model_names_refs);
    model_row.set_model(Some(&models));

    // Set current model
    let current_model = &state.borrow().settings.model;
    let model_index = MODELS
        .iter()
        .position(|(id, _, _)| *id == current_model)
        .unwrap_or(0);
    model_row.set_selected(model_index as u32);

    let state_clone = state.clone();
    model_row.connect_selected_notify(move |row| {
        let idx = row.selected() as usize;
        if let Some((model_id, _, _)) = MODELS.get(idx) {
            state_clone.borrow_mut().settings.model = model_id.to_string();
        }
    });

    ai_group.add(&model_row);
    page.add(&ai_group);

    // STT Provider group
    let stt_group = adw::PreferencesGroup::new();
    stt_group.set_title("Speech-to-Text");
    stt_group.set_description(Some("Configure STT for audio transcription"));

    // STT API Key
    let stt_key_row = adw::PasswordEntryRow::new();
    stt_key_row.set_title("STT API Key");
    stt_key_row.set_text(&state.borrow().settings.stt_api_key);

    let state_clone = state.clone();
    stt_key_row.connect_changed(move |row| {
        state_clone.borrow_mut().settings.stt_api_key = row.text().to_string();
    });

    stt_group.add(&stt_key_row);

    // STT Model
    let stt_model_row = adw::ComboRow::new();
    stt_model_row.set_title("STT Model");
    let stt_models = gtk4::StringList::new(&["whisper-large-v3-turbo", "whisper-large-v3"]);
    stt_model_row.set_model(Some(&stt_models));

    let current_stt = &state.borrow().settings.stt_model;
    let stt_index = if current_stt == "whisper-large-v3" {
        1
    } else {
        0
    };
    stt_model_row.set_selected(stt_index);

    let state_clone = state.clone();
    stt_model_row.connect_selected_notify(move |row| {
        let model = if row.selected() == 1 {
            "whisper-large-v3"
        } else {
            "whisper-large-v3-turbo"
        };
        state_clone.borrow_mut().settings.stt_model = model.to_string();
    });

    stt_group.add(&stt_model_row);
    page.add(&stt_group);

    // Appearance group
    let appearance_group = adw::PreferencesGroup::new();
    appearance_group.set_title("Appearance");

    let dark_mode_row = adw::SwitchRow::new();
    dark_mode_row.set_title("Dark Mode");
    dark_mode_row.set_subtitle("Use dark theme");
    dark_mode_row.set_active(state.borrow().settings.dark_mode);

    let state_clone = state.clone();
    dark_mode_row.connect_active_notify(move |row| {
        state_clone.borrow_mut().settings.dark_mode = row.is_active();
    });

    appearance_group.add(&dark_mode_row);
    page.add(&appearance_group);

    // Save button
    let save_group = adw::PreferencesGroup::new();

    let save_btn = gtk4::Button::with_label("Save Settings");
    save_btn.add_css_class("suggested-action");
    save_btn.set_margin_top(16);

    let state_clone = state.clone();
    save_btn.connect_clicked(move |_| {
        if let Err(e) = state_clone.borrow().save() {
            tracing::error!("Failed to save settings: {}", e);
        }
    });

    save_group.add(&save_btn);
    page.add(&save_group);

    vbox.append(&page);
    vbox
}
