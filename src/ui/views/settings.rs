// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Settings View - API keys and configuration

use crate::core::models::{models_for_provider, stt_models_for_provider};
use crate::core::state::{AiProvider, AppState, SttProvider};
use adw::prelude::*;
use gtk4::{glib, ApplicationWindow, Box as GtkBox, PolicyType, ScrolledWindow, StringList};
use libadwaita as adw;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

/// Schedule a debounced save: cancels any pending timer and arms a new 500ms one.
fn schedule_autosave(debounce: &Rc<Cell<Option<glib::SourceId>>>, state: &Rc<RefCell<AppState>>) {
    if let Some(id) = debounce.take() {
        id.remove();
    }
    let state_save = state.clone();
    let debounce_reset = debounce.clone();
    let source_id = glib::timeout_add_local_once(Duration::from_millis(500), move || {
        debounce_reset.set(None);
        if let Err(e) = state_save.borrow().save() {
            tracing::error!("Failed to auto-save settings: {}", e);
        }
    });
    debounce.set(Some(source_id));
}

/// Rebuild model combo row items for the given provider
fn rebuild_model_list(model_row: &adw::ComboRow, provider: AiProvider, current_model: &str) {
    let models = models_for_provider(provider);
    let names: Vec<String> = models
        .iter()
        .map(|m| {
            if m.supports_vision {
                format!("👁\u{fe0f} {}", m.display_name)
            } else {
                m.display_name.to_string()
            }
        })
        .collect();
    let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
    model_row.set_model(Some(&StringList::new(&refs)));

    let idx = models
        .iter()
        .position(|m| m.id == current_model)
        .unwrap_or(0);
    model_row.set_selected(idx as u32);
}

/// Rebuild STT model combo row items for the given provider
fn rebuild_stt_model_list(model_row: &adw::ComboRow, provider: SttProvider, current_model: &str) {
    let models = stt_models_for_provider(provider);
    let names: Vec<&str> = models.iter().map(|m| m.display_name).collect();
    model_row.set_model(Some(&StringList::new(&names)));

    let idx = models
        .iter()
        .position(|m| m.id == current_model)
        .unwrap_or(0);
    model_row.set_selected(idx as u32);
}

/// Build the settings view
pub fn build(window: &ApplicationWindow, state: Rc<RefCell<AppState>>) -> GtkBox {
    let vbox = GtkBox::new(gtk4::Orientation::Vertical, 0);
    let page = adw::PreferencesPage::new();

    // Shared debounce timer — all setting changes funnel through this one timer,
    // so a burst of edits (e.g. typing an API key) produces only one disk write.
    let save_debounce: Rc<Cell<Option<glib::SourceId>>> = Rc::new(Cell::new(None));

    // Guard flag to prevent recursive updates when switching provider
    let updating = Rc::new(Cell::new(false));

    // AI Provider group
    let ai_group = adw::PreferencesGroup::new();
    ai_group.set_title("AI Provider");

    // Provider selector
    let provider_row = adw::ComboRow::new();
    provider_row.set_title("Provider");
    let provider_names: Vec<&str> = AiProvider::ALL.iter().map(|p| p.display_name()).collect();
    provider_row.set_model(Some(&StringList::new(&provider_names)));
    let current_provider = state.borrow().settings.provider;
    let provider_idx = AiProvider::ALL
        .iter()
        .position(|&p| p == current_provider)
        .unwrap_or(0);
    provider_row.set_selected(provider_idx as u32);
    ai_group.add(&provider_row);

    // API Key (shows key for selected provider)
    let api_key_row = adw::PasswordEntryRow::new();
    api_key_row.set_title("API Key");
    api_key_row.set_text(state.borrow().settings.active_api_key());

    let state_c = state.clone();
    let debounce = save_debounce.clone();
    let updating_c = updating.clone();
    api_key_row.connect_changed(move |row| {
        if updating_c.get() {
            return;
        }
        state_c
            .borrow_mut()
            .settings
            .set_active_api_key(row.text().to_string());
        schedule_autosave(&debounce, &state_c);
    });
    ai_group.add(&api_key_row);

    // Model selection (shows models for selected provider)
    let model_row = adw::ComboRow::new();
    model_row.set_title("Model");
    model_row.set_subtitle("Vision models marked with 👁\u{fe0f}");
    rebuild_model_list(
        &model_row,
        current_provider,
        state.borrow().settings.active_model(),
    );

    let state_c = state.clone();
    let debounce = save_debounce.clone();
    let updating_c = updating.clone();
    model_row.connect_selected_notify(move |row| {
        if updating_c.get() {
            return;
        }
        let provider = state_c.borrow().settings.provider;
        let models = models_for_provider(provider);
        let idx = row.selected() as usize;
        if let Some(m) = models.get(idx) {
            state_c
                .borrow_mut()
                .settings
                .set_active_model(m.id.to_string());
        }
        schedule_autosave(&debounce, &state_c);
    });
    ai_group.add(&model_row);

    // Wire provider selector to update API key + model dynamically
    let state_c = state.clone();
    let debounce = save_debounce.clone();
    let api_key_c = api_key_row.clone();
    let model_c = model_row.clone();
    let updating_c = updating.clone();
    provider_row.connect_selected_notify(move |row| {
        let idx = row.selected() as usize;
        let provider = AiProvider::ALL.get(idx).copied().unwrap_or_default();

        updating_c.set(true);
        state_c.borrow_mut().settings.provider = provider;
        api_key_c.set_text(state_c.borrow().settings.active_api_key());
        rebuild_model_list(&model_c, provider, state_c.borrow().settings.active_model());
        updating_c.set(false);

        schedule_autosave(&debounce, &state_c);
    });

    page.add(&ai_group);

    // STT Provider group
    let stt_group = adw::PreferencesGroup::new();
    stt_group.set_title("Speech-to-Text");

    let stt_updating = Rc::new(Cell::new(false));

    // STT Provider selector
    let stt_provider_row = adw::ComboRow::new();
    stt_provider_row.set_title("STT Provider");
    let stt_provider_names: Vec<&str> = SttProvider::ALL.iter().map(|p| p.display_name()).collect();
    stt_provider_row.set_model(Some(&StringList::new(&stt_provider_names)));
    let current_stt_provider = state.borrow().settings.stt_provider;
    let stt_provider_idx = SttProvider::ALL
        .iter()
        .position(|&p| p == current_stt_provider)
        .unwrap_or(0);
    stt_provider_row.set_selected(stt_provider_idx as u32);
    stt_group.add(&stt_provider_row);

    // STT API Key
    let stt_key_row = adw::PasswordEntryRow::new();
    stt_key_row.set_title("STT API Key");
    stt_key_row.set_text(state.borrow().settings.active_stt_key());

    let state_c = state.clone();
    let debounce = save_debounce.clone();
    let stt_updating_c = stt_updating.clone();
    stt_key_row.connect_changed(move |row| {
        if stt_updating_c.get() {
            return;
        }
        state_c
            .borrow_mut()
            .settings
            .set_active_stt_key(row.text().to_string());
        schedule_autosave(&debounce, &state_c);
    });
    stt_group.add(&stt_key_row);

    // STT Model
    let stt_model_row = adw::ComboRow::new();
    stt_model_row.set_title("STT Model");
    rebuild_stt_model_list(
        &stt_model_row,
        current_stt_provider,
        state.borrow().settings.active_stt_model(),
    );

    let state_c = state.clone();
    let debounce = save_debounce.clone();
    let stt_updating_c = stt_updating.clone();
    stt_model_row.connect_selected_notify(move |row| {
        if stt_updating_c.get() {
            return;
        }
        let provider = state_c.borrow().settings.stt_provider;
        let models = stt_models_for_provider(provider);
        let idx = row.selected() as usize;
        if let Some(m) = models.get(idx) {
            state_c
                .borrow_mut()
                .settings
                .set_active_stt_model(m.id.to_string());
        }
        schedule_autosave(&debounce, &state_c);
    });
    stt_group.add(&stt_model_row);

    // Wire STT provider selector
    let state_c = state.clone();
    let debounce = save_debounce.clone();
    let stt_key_c = stt_key_row.clone();
    let stt_model_c = stt_model_row.clone();
    let stt_updating_c = stt_updating.clone();
    stt_provider_row.connect_selected_notify(move |row| {
        let idx = row.selected() as usize;
        let provider = SttProvider::ALL.get(idx).copied().unwrap_or_default();

        stt_updating_c.set(true);
        state_c.borrow_mut().settings.stt_provider = provider;
        stt_key_c.set_text(state_c.borrow().settings.active_stt_key());
        rebuild_stt_model_list(
            &stt_model_c,
            provider,
            state_c.borrow().settings.active_stt_model(),
        );
        stt_updating_c.set(false);

        schedule_autosave(&debounce, &state_c);
    });

    page.add(&stt_group);

    // Live Assistant group
    let live_group = adw::PreferencesGroup::new();
    live_group.set_title("Live Assistant");

    let chunk_row = adw::ActionRow::new();
    chunk_row.set_title("Chunk Duration");
    chunk_row.set_subtitle("System audio chunk size in seconds");
    let chunk_scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 3.0, 10.0, 1.0);
    chunk_scale.set_value(state.borrow().settings.live_chunk_secs as f64);
    chunk_scale.set_draw_value(true);
    chunk_scale.set_value_pos(gtk4::PositionType::Right);
    chunk_scale.set_hexpand(true);
    chunk_scale.set_width_request(200);
    chunk_scale.set_format_value_func(|_, value| format!("{:.0}s", value));

    let state_c = state.clone();
    let debounce = save_debounce.clone();
    chunk_scale.connect_value_changed(move |scale| {
        state_c.borrow_mut().settings.live_chunk_secs = scale.value().round() as u64;
        schedule_autosave(&debounce, &state_c);
    });
    chunk_row.add_suffix(&chunk_scale);
    chunk_row.set_activatable_widget(Some(&chunk_scale));
    live_group.add(&chunk_row);

    let auto_suggest_row = adw::SwitchRow::new();
    auto_suggest_row.set_title("Auto Suggest");
    auto_suggest_row.set_subtitle("Answer automatically after speech pauses");
    auto_suggest_row.set_active(state.borrow().settings.live_auto_suggest);

    let state_c = state.clone();
    let debounce = save_debounce.clone();
    auto_suggest_row.connect_active_notify(move |row| {
        state_c.borrow_mut().settings.live_auto_suggest = row.is_active();
        schedule_autosave(&debounce, &state_c);
    });
    live_group.add(&auto_suggest_row);

    let debug_row = adw::SwitchRow::new();
    debug_row.set_title("Transcript Debug");
    debug_row.set_subtitle("Show recent live transcript in the chat status area");
    debug_row.set_active(state.borrow().settings.live_debug_transcript);

    let state_c = state.clone();
    let debounce = save_debounce.clone();
    debug_row.connect_active_notify(move |row| {
        state_c.borrow_mut().settings.live_debug_transcript = row.is_active();
        schedule_autosave(&debounce, &state_c);
    });
    live_group.add(&debug_row);

    page.add(&live_group);

    // Appearance group
    let appearance_group = adw::PreferencesGroup::new();
    appearance_group.set_title("Appearance");

    let dark_mode_row = adw::SwitchRow::new();
    dark_mode_row.set_title("Dark Mode");
    dark_mode_row.set_subtitle("Use dark theme");
    dark_mode_row.set_active(state.borrow().settings.dark_mode);

    let state_c = state.clone();
    let debounce = save_debounce.clone();
    dark_mode_row.connect_active_notify(move |row| {
        let is_dark = row.is_active();
        state_c.borrow_mut().settings.dark_mode = is_dark;

        // Apply theme immediately
        if let Some(display) = gtk4::gdk::Display::default() {
            let style_manager = adw::StyleManager::for_display(&display);
            style_manager.set_color_scheme(if is_dark {
                adw::ColorScheme::ForceDark
            } else {
                adw::ColorScheme::ForceLight
            });
        }
        schedule_autosave(&debounce, &state_c);
    });

    appearance_group.add(&dark_mode_row);

    // Window Opacity
    let opacity_row = adw::ActionRow::new();
    opacity_row.set_title("Window Opacity");
    opacity_row.set_subtitle("Transparency level (100% = opaque)");

    let opacity_scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 0.3, 1.0, 0.05);
    opacity_scale.set_value(state.borrow().settings.window_opacity);
    opacity_scale.set_draw_value(true);
    opacity_scale.set_value_pos(gtk4::PositionType::Right);
    opacity_scale.set_hexpand(true);
    opacity_scale.set_width_request(200);
    opacity_scale.set_format_value_func(|_, value| format!("{:.0}%", value * 100.0));

    let window_clone = window.clone();
    let state_c = state.clone();
    let debounce = save_debounce.clone();
    opacity_scale.connect_value_changed(move |scale| {
        let opacity = scale.value();
        state_c.borrow_mut().settings.window_opacity = opacity;
        window_clone.set_opacity(opacity);
        schedule_autosave(&debounce, &state_c);
    });

    opacity_row.add_suffix(&opacity_scale);
    opacity_row.set_activatable_widget(Some(&opacity_scale));
    appearance_group.add(&opacity_row);

    page.add(&appearance_group);

    // Wrap in ScrolledWindow to allow vertical resize
    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .vexpand(true)
        .child(&page)
        .build();

    vbox.append(&scroll);
    vbox
}
