// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Settings View - API keys and configuration

use crate::core::api::list_models;
use crate::core::models::{
    default_vision_model, models_for_provider, stt_models_for_provider, ModelInfo,
};
use crate::core::state::{
    AiProvider, AppState, SttProvider, LIVE_CHUNK_MAX_SECS, LIVE_CHUNK_MIN_SECS,
};
use adw::prelude::*;
use gtk4::{gio, glib, Box as GtkBox, PolicyType, ScrolledWindow, StringList};
use libadwaita as adw;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

const CUSTOM_MODEL: &str = "Custom ID";
const VISION_HINT: &str = "Vision models marked with 👁\u{fe0f}";

type Debounce = Rc<Cell<Option<glib::SourceId>>>;

/// Schedule a debounced save: cancels any pending timer and arms a new 500ms one.
fn schedule_autosave(debounce: &Debounce, state: &Rc<RefCell<AppState>>) {
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

/// Model list plus an editable ID, for models the built-in catalog does not have.
#[derive(Clone)]
struct ModelPicker {
    list: adw::ComboRow,
    id: adw::EntryRow,
    ids: Rc<RefCell<Vec<String>>>,
    syncing: Rc<Cell<bool>>,
}

impl ModelPicker {
    fn new(group: &adw::PreferencesGroup, title: &str) -> Self {
        let list = adw::ComboRow::new();
        list.set_title(title);
        let id = adw::EntryRow::new();
        id.set_title(&format!("{title} ID"));
        group.add(&list);
        group.add(&id);
        Self {
            list,
            id,
            ids: Rc::default(),
            syncing: Rc::default(),
        }
    }

    /// Replace the choices without reporting a change.
    fn show(&self, choices: Vec<(String, String)>, current: &str) {
        self.fill(choices, current, true);
    }

    /// Offer only the models a server serves, `current` among them. The ID row is
    /// hidden: there is nothing to type when the server tells what it has.
    fn show_served(&self, ids: Vec<String>, current: &str) {
        let choices = ids.into_iter().map(|id| (id.clone(), id)).collect();
        self.fill(choices, current, false);
    }

    fn fill(&self, choices: Vec<(String, String)>, current: &str, custom: bool) {
        self.syncing.set(true);
        let mut labels: Vec<&str> = choices.iter().map(|(_, label)| label.as_str()).collect();
        if custom {
            labels.push(CUSTOM_MODEL);
        }
        self.id.set_visible(custom);
        self.list.set_model(Some(&StringList::new(&labels)));
        *self.ids.borrow_mut() = choices.into_iter().map(|(id, _)| id).collect();
        self.select(current);
        self.id.set_text(current);
        self.syncing.set(false);
    }

    fn select(&self, current: &str) {
        let ids = self.ids.borrow();
        let idx = ids.iter().position(|id| id == current).unwrap_or(ids.len());
        self.list.set_selected(idx as u32);
    }

    fn connect_changed(&self, on_change: impl Fn(&str) + 'static) {
        let on_change = Rc::new(on_change);

        let picker = self.clone();
        let on_list_change = on_change.clone();
        self.list.connect_selected_notify(move |row| {
            if picker.syncing.get() {
                return;
            }
            let selected = picker.ids.borrow().get(row.selected() as usize).cloned();
            if let Some(id) = selected {
                picker.syncing.set(true);
                picker.id.set_text(&id);
                picker.syncing.set(false);
                on_list_change(&id);
            } else {
                picker.id.grab_focus();
            }
        });

        let picker = self.clone();
        self.id.connect_changed(move |row| {
            let text = row.text();
            let id = text.trim();
            if picker.syncing.get() || id.is_empty() {
                return;
            }
            picker.syncing.set(true);
            picker.select(id);
            picker.syncing.set(false);
            on_change(id);
        });
    }
}

fn labeled(model: &ModelInfo) -> (String, String) {
    let label = if model.supports_vision {
        format!("👁\u{fe0f} {}", model.display_name)
    } else {
        model.display_name.to_string()
    };
    (model.id.to_string(), label)
}

fn llm_choices(provider: AiProvider) -> Vec<(String, String)> {
    models_for_provider(provider).iter().map(labeled).collect()
}

fn vision_choices(provider: AiProvider) -> Vec<(String, String)> {
    models_for_provider(provider)
        .iter()
        .filter(|m| m.supports_vision)
        .map(labeled)
        .collect()
}

fn stt_choices(provider: SttProvider) -> Vec<(String, String)> {
    stt_models_for_provider(provider)
        .iter()
        .map(|m| (m.id.to_string(), m.display_name.to_string()))
        .collect()
}

/// Rows that only an OpenAI-compatible server uses, and the labels that differ for it
#[derive(Clone)]
struct ServerRows {
    base_url: adw::EntryRow,
    api_key: adw::PasswordEntryRow,
    vision: adw::SwitchRow,
    model: ModelPicker,
}

impl ServerRows {
    /// Show the rows for `provider` and offer its models: a server is asked for its own.
    fn show(&self, state: &Rc<RefCell<AppState>>, debounce: &Debounce) {
        let settings = &state.borrow().settings;
        let server = settings.provider == AiProvider::OpenAICompatible;
        self.base_url.set_visible(server);
        self.vision.set_visible(server);
        self.api_key.set_show_apply_button(server);
        self.api_key.set_title(if server {
            "API Key (optional)"
        } else {
            "API Key"
        });
        self.model
            .show(llm_choices(settings.provider), settings.active_model());
        if server {
            self.load_models(state, debounce);
        } else {
            self.model.list.set_subtitle(VISION_HINT);
        }
    }

    /// Ask the server for its models. A reply for a URL that is no longer set is dropped.
    fn load_models(&self, state: &Rc<RefCell<AppState>>, debounce: &Debounce) {
        let (base_url, api_key) = {
            let settings = &state.borrow().settings;
            (
                settings.compatible_base_url.clone(),
                settings.compatible_api_key.clone(),
            )
        };
        self.model
            .list
            .set_subtitle(&format!("Asking {base_url} for its models…"));

        let rows = self.clone();
        let state = state.clone();
        let debounce = debounce.clone();
        glib::spawn_future_local(async move {
            let url = base_url.clone();
            let result = gio::spawn_blocking(move || list_models(&url, &api_key)).await;
            let current = {
                let settings = &state.borrow().settings;
                if settings.provider != AiProvider::OpenAICompatible
                    || settings.compatible_base_url != base_url
                {
                    return;
                }
                settings.compatible_model.clone()
            };
            // Without a list the ID row stays, so the user can still type one.
            let ids = match result {
                Ok(Ok(ids)) if !ids.is_empty() => ids,
                Ok(Ok(_)) => {
                    let note = "The server lists no models. Enter the ID below";
                    return rows.model.list.set_subtitle(note);
                }
                Ok(Err(e)) => {
                    let note = format!("{e}. Fix the URL or enter the ID below");
                    return rows.model.list.set_subtitle(&note);
                }
                Err(_) => return rows.model.list.set_subtitle("Model list failed"),
            };
            rows.model
                .list
                .set_subtitle(&format!("Served by {base_url}"));
            // A fresh setup, or a model this server does not have: take its first one.
            let current = if ids.contains(&current) {
                current
            } else {
                let first = ids[0].clone();
                state.borrow_mut().settings.compatible_model = first.clone();
                schedule_autosave(&debounce, &state);
                first
            };
            rows.model.show_served(ids, &current);
        });
    }
}

/// Build the settings view
pub fn build(state: Rc<RefCell<AppState>>) -> GtkBox {
    let vbox = GtkBox::new(gtk4::Orientation::Vertical, 0);
    let page = adw::PreferencesPage::new();

    // Shared debounce timer — all setting changes funnel through this one timer,
    // so a burst of edits (e.g. typing an API key) produces only one disk write.
    let save_debounce: Debounce = Rc::default();

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

    // Server URL, for an OpenAI-compatible provider only
    let base_url_row = adw::EntryRow::new();
    base_url_row.set_title("Server URL");
    base_url_row.set_show_apply_button(true);
    base_url_row.set_text(&state.borrow().settings.compatible_base_url);
    ai_group.add(&base_url_row);

    // API Key (shows key for selected provider)
    let api_key_row = adw::PasswordEntryRow::new();
    api_key_row.set_text(state.borrow().settings.active_api_key());
    ai_group.add(&api_key_row);

    // Model selection (shows models for selected provider)
    let model_picker = ModelPicker::new(&ai_group, "Model");

    let vision_row = adw::SwitchRow::new();
    vision_row.set_title("Reads Images");
    vision_row.set_subtitle("Off sends screenshots to the image model below");
    vision_row.set_active(state.borrow().settings.compatible_vision);
    ai_group.add(&vision_row);

    let server_rows = ServerRows {
        base_url: base_url_row.clone(),
        api_key: api_key_row.clone(),
        vision: vision_row.clone(),
        model: model_picker.clone(),
    };
    server_rows.show(&state, &save_debounce);

    let state_c = state.clone();
    let debounce = save_debounce.clone();
    base_url_row.connect_changed(move |row| {
        state_c.borrow_mut().settings.compatible_base_url = row.text().trim().to_string();
        schedule_autosave(&debounce, &state_c);
    });

    // Applying the URL or the key asks the server for its models again.
    let state_c = state.clone();
    let debounce = save_debounce.clone();
    let rows_c = server_rows.clone();
    base_url_row.connect_apply(move |_| rows_c.load_models(&state_c, &debounce));

    let state_c = state.clone();
    let debounce = save_debounce.clone();
    let rows_c = server_rows.clone();
    api_key_row.connect_apply(move |_| rows_c.load_models(&state_c, &debounce));

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

    let state_c = state.clone();
    let debounce = save_debounce.clone();
    model_picker.connect_changed(move |id| {
        state_c
            .borrow_mut()
            .settings
            .set_active_model(id.to_string());
        schedule_autosave(&debounce, &state_c);
    });

    let state_c = state.clone();
    let debounce = save_debounce.clone();
    vision_row.connect_active_notify(move |row| {
        state_c.borrow_mut().settings.compatible_vision = row.is_active();
        schedule_autosave(&debounce, &state_c);
    });

    // Wire provider selector to update API key + model dynamically
    let state_c = state.clone();
    let debounce = save_debounce.clone();
    let updating_c = updating.clone();
    provider_row.connect_selected_notify(move |row| {
        let idx = row.selected() as usize;
        let provider = AiProvider::ALL.get(idx).copied().unwrap_or_default();

        updating_c.set(true);
        state_c.borrow_mut().settings.provider = provider;
        server_rows
            .api_key
            .set_text(state_c.borrow().settings.active_api_key());
        server_rows.show(&state_c, &debounce);
        updating_c.set(false);

        schedule_autosave(&debounce, &state_c);
    });

    page.add(&ai_group);

    // Model for requests with images when the selected model cannot read them
    let vision_group = adw::PreferencesGroup::new();
    vision_group.set_title("Image Model");
    vision_group.set_description(Some(
        "Used when the selected model cannot read images. It uses the API key of its \
         provider: select that provider above to enter the key.",
    ));

    let vision_provider_row = adw::ComboRow::new();
    vision_provider_row.set_title("Provider");
    let cloud_names: Vec<&str> = AiProvider::CLOUD.iter().map(|p| p.display_name()).collect();
    vision_provider_row.set_model(Some(&StringList::new(&cloud_names)));
    let current_vision_provider = state.borrow().settings.vision_fallback_provider;
    let vision_provider_idx = AiProvider::CLOUD
        .iter()
        .position(|&p| p == current_vision_provider)
        .unwrap_or(0);
    vision_provider_row.set_selected(vision_provider_idx as u32);
    vision_group.add(&vision_provider_row);

    let vision_picker = ModelPicker::new(&vision_group, "Model");
    vision_picker.show(
        vision_choices(current_vision_provider),
        &state.borrow().settings.vision_fallback_model,
    );

    let state_c = state.clone();
    let debounce = save_debounce.clone();
    vision_picker.connect_changed(move |id| {
        state_c.borrow_mut().settings.vision_fallback_model = id.to_string();
        schedule_autosave(&debounce, &state_c);
    });

    let state_c = state.clone();
    let debounce = save_debounce.clone();
    let vision_picker_c = vision_picker.clone();
    vision_provider_row.connect_selected_notify(move |row| {
        let idx = row.selected() as usize;
        let provider = AiProvider::CLOUD.get(idx).copied().unwrap_or_default();
        let model = default_vision_model(provider);
        {
            let mut state = state_c.borrow_mut();
            state.settings.vision_fallback_provider = provider;
            state.settings.vision_fallback_model = model.to_string();
        }
        vision_picker_c.show(vision_choices(provider), model);
        schedule_autosave(&debounce, &state_c);
    });

    page.add(&vision_group);

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
    let stt_model_picker = ModelPicker::new(&stt_group, "STT Model");
    stt_model_picker.show(
        stt_choices(current_stt_provider),
        state.borrow().settings.active_stt_model(),
    );

    let state_c = state.clone();
    let debounce = save_debounce.clone();
    stt_model_picker.connect_changed(move |id| {
        state_c
            .borrow_mut()
            .settings
            .set_active_stt_model(id.to_string());
        schedule_autosave(&debounce, &state_c);
    });

    page.add(&stt_group);

    // Live Assistant group
    let live_group = adw::PreferencesGroup::new();
    live_group.set_title("Live Assistant");

    let chunk_row = adw::ActionRow::new();
    chunk_row.set_title("Chunk Duration");
    chunk_row.set_subtitle("System audio chunk size in seconds\nGroq bills a minimum of 10s");
    let chunk_scale = gtk4::Scale::with_range(
        gtk4::Orientation::Horizontal,
        LIVE_CHUNK_MIN_SECS as f64,
        LIVE_CHUNK_MAX_SECS as f64,
        1.0,
    );
    chunk_scale.set_value(state.borrow().settings.effective_live_chunk_secs() as f64);
    chunk_scale.set_draw_value(true);
    chunk_scale.set_value_pos(gtk4::PositionType::Right);
    chunk_scale.set_hexpand(true);
    chunk_scale.set_width_request(200);
    chunk_scale.set_format_value_func(|_, value| format!("{:.0}s", value));

    let state_c = state.clone();
    let debounce = save_debounce.clone();
    chunk_scale.connect_value_changed(move |scale| {
        let mut state = state_c.borrow_mut();
        let value = scale.value().round() as u64;
        state.settings.live_chunk_secs = value.clamp(LIVE_CHUNK_MIN_SECS, LIVE_CHUNK_MAX_SECS);
        schedule_autosave(&debounce, &state_c);
    });
    chunk_row.add_suffix(&chunk_scale);
    chunk_row.set_activatable_widget(Some(&chunk_scale));
    live_group.add(&chunk_row);

    // Wire STT provider selector after chunk controls exist.
    let state_c = state.clone();
    let debounce = save_debounce.clone();
    let stt_key_c = stt_key_row.clone();
    let stt_model_c = stt_model_picker.clone();
    let stt_updating_c = stt_updating.clone();
    stt_provider_row.connect_selected_notify(move |row| {
        let idx = row.selected() as usize;
        let provider = SttProvider::ALL.get(idx).copied().unwrap_or_default();

        stt_updating_c.set(true);
        {
            let mut state = state_c.borrow_mut();
            state.settings.stt_provider = provider;
        }
        stt_key_c.set_text(state_c.borrow().settings.active_stt_key());
        stt_model_c.show(
            stt_choices(provider),
            state_c.borrow().settings.active_stt_model(),
        );
        stt_updating_c.set(false);

        schedule_autosave(&debounce, &state_c);
    });

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
        crate::ui::theme::apply(is_dark, state_c.borrow().settings.window_opacity);
        schedule_autosave(&debounce, &state_c);
    });

    appearance_group.add(&dark_mode_row);

    // Background opacity
    let opacity_row = adw::ActionRow::new();
    opacity_row.set_title("Background Opacity");
    opacity_row.set_subtitle("Text stays fully opaque");

    let opacity_scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 0.3, 1.0, 0.05);
    opacity_scale.set_value(state.borrow().settings.window_opacity);
    opacity_scale.set_draw_value(true);
    opacity_scale.set_value_pos(gtk4::PositionType::Right);
    opacity_scale.set_hexpand(true);
    opacity_scale.set_width_request(200);
    opacity_scale.set_format_value_func(|_, value| format!("{:.0}%", value * 100.0));

    let state_c = state.clone();
    let debounce = save_debounce.clone();
    opacity_scale.connect_value_changed(move |scale| {
        let opacity = scale.value();
        state_c.borrow_mut().settings.window_opacity = opacity;
        crate::ui::theme::apply(state_c.borrow().settings.dark_mode, opacity);
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
