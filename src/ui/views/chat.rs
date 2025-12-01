// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Chat View - AI interaction with Vision + Audio

use crate::core::api::{groq::ImageData, GroqClient, WhisperClient};
use crate::core::audio::{AudioRecorder, ListenRecorder};
use crate::core::capture;
use crate::core::state::{AppState, Message};
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, Entry, Label, Orientation, PolicyType, ScrolledWindow, TextView,
    WrapMode,
};
use std::cell::RefCell;
use std::rc::Rc;

/// Build the chat view
pub fn build(state: Rc<RefCell<AppState>>) -> GtkBox {
    let vbox = GtkBox::new(Orientation::Vertical, 8);
    vbox.set_margin_top(8);
    vbox.set_margin_bottom(8);
    vbox.set_margin_start(8);
    vbox.set_margin_end(8);

    // Shared state
    let recorder = Rc::new(RefCell::new(AudioRecorder::new()));
    let listener = Rc::new(RefCell::new(ListenRecorder::new()));
    let attached_image: Rc<RefCell<Option<ImageData>>> = Rc::new(RefCell::new(None));

    // Status label
    let status_label = Label::new(None);
    status_label.set_halign(gtk4::Align::Start);
    status_label.add_css_class("dim-label");
    status_label.set_visible(false);
    vbox.append(&status_label);

    // Attachment indicator
    let attachment_label = Label::new(None);
    attachment_label.set_halign(gtk4::Align::Start);
    attachment_label.add_css_class("accent");
    attachment_label.set_visible(false);
    vbox.append(&attachment_label);

    // Chat display
    let chat_view = TextView::new();
    chat_view.set_editable(false);
    chat_view.set_cursor_visible(false);
    chat_view.set_wrap_mode(WrapMode::Word);
    chat_view.set_vexpand(true);
    chat_view.set_top_margin(8);
    chat_view.set_bottom_margin(8);
    chat_view.set_left_margin(8);
    chat_view.set_right_margin(8);

    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .vexpand(true)
        .child(&chat_view)
        .build();

    vbox.append(&scroll);
    update_chat_display(&chat_view, &state.borrow());

    // Input area
    let input_box = GtkBox::new(Orientation::Horizontal, 8);

    let entry = Entry::new();
    entry.set_hexpand(true);
    entry.set_placeholder_text(Some("Type your message..."));

    let send_btn = Button::with_label("Send");
    send_btn.add_css_class("suggested-action");

    // Send handlers
    {
        let entry_c = entry.clone();
        let state_c = state.clone();
        let chat_view_c = chat_view.clone();
        let status_c = status_label.clone();
        let send_btn_c = send_btn.clone();
        let image_c = attached_image.clone();
        let attach_label_c = attachment_label.clone();

        send_btn.connect_clicked(move |_| {
            send_message(
                &entry_c,
                &state_c,
                &chat_view_c,
                &status_c,
                &send_btn_c,
                &image_c,
                &attach_label_c,
            );
        });
    }

    {
        let state_c = state.clone();
        let chat_view_c = chat_view.clone();
        let status_c = status_label.clone();
        let send_btn_c = send_btn.clone();
        let image_c = attached_image.clone();
        let attach_label_c = attachment_label.clone();

        entry.connect_activate(move |entry| {
            send_message(
                entry,
                &state_c,
                &chat_view_c,
                &status_c,
                &send_btn_c,
                &image_c,
                &attach_label_c,
            );
        });
    }

    input_box.append(&entry);
    input_box.append(&send_btn);
    vbox.append(&input_box);

    // Action buttons
    let buttons_box = GtkBox::new(Orientation::Horizontal, 8);
    buttons_box.set_halign(gtk4::Align::Center);
    buttons_box.set_margin_top(8);

    // Audio button
    let audio_btn = Button::with_label("🎤 Record");
    audio_btn.set_tooltip_text(Some("Record and transcribe audio"));

    {
        let recorder_c = recorder.clone();
        let state_c = state.clone();
        let status_c = status_label.clone();
        let audio_btn_c = audio_btn.clone();
        let entry_c = entry.clone();

        audio_btn.connect_clicked(move |_| {
            handle_audio_click(&recorder_c, &state_c, &status_c, &audio_btn_c, &entry_c);
        });
    }

    // Listen button (system audio capture)
    let listen_btn = Button::with_label("🎧 Listen");
    listen_btn.set_tooltip_text(Some("Record system audio (what you hear)"));

    {
        let listener_c = listener.clone();
        let state_c = state.clone();
        let status_c = status_label.clone();
        let listen_btn_c = listen_btn.clone();
        let entry_c = entry.clone();

        listen_btn.connect_clicked(move |_| {
            handle_listen_click(&listener_c, &state_c, &status_c, &listen_btn_c, &entry_c);
        });
    }

    // Screenshot button
    let screenshot_btn = Button::with_label("📷 Screenshot");
    screenshot_btn.set_tooltip_text(Some("Take screenshot and attach to chat"));

    {
        let image_c = attached_image.clone();
        let status_c = status_label.clone();
        let attach_label_c = attachment_label.clone();

        screenshot_btn.connect_clicked(move |_| {
            // Stealth window is not captured by grim/gnome-screenshot
            handle_screenshot_click(&image_c, &status_c, &attach_label_c);
        });
    }

    // Clear button
    let clear_btn = Button::with_label("🗑️ Clear");
    clear_btn.set_tooltip_text(Some("Clear chat and attachments"));

    {
        let state_c = state.clone();
        let chat_view_c = chat_view.clone();
        let image_c = attached_image.clone();
        let attach_label_c = attachment_label.clone();

        clear_btn.connect_clicked(move |_| {
            state_c.borrow_mut().current_messages.clear();
            *image_c.borrow_mut() = None;
            attach_label_c.set_visible(false);
            update_chat_display(&chat_view_c, &state_c.borrow());
        });
    }

    buttons_box.append(&audio_btn);
    buttons_box.append(&listen_btn);
    buttons_box.append(&screenshot_btn);
    buttons_box.append(&clear_btn);
    vbox.append(&buttons_box);

    vbox
}

/// Transcribe audio and insert result into entry field
fn transcribe_and_insert(
    audio_path: std::path::PathBuf,
    state: &Rc<RefCell<AppState>>,
    status_label: &Label,
    entry: &Entry,
    prefix: Option<&str>,
) {
    let (stt_key, stt_model) = {
        let s = state.borrow();
        (s.settings.stt_api_key.clone(), s.settings.stt_model.clone())
    };

    if stt_key.is_empty() {
        status_label.set_text("⚠️ Set STT API key in Settings");
        return;
    }

    status_label.set_text("⏳ Transcribing...");
    status_label.set_visible(true);

    let status_c = status_label.clone();
    let entry_c = entry.clone();
    let prefix_owned = prefix.map(|s| s.to_string());

    glib::spawn_future_local(async move {
        let result = gio::spawn_blocking(move || {
            let client = WhisperClient::new(&stt_key, &stt_model);
            client.transcribe(&audio_path).map_err(|e| e.to_string())
        })
        .await;

        match result {
            Ok(Ok(text)) => {
                let formatted = match &prefix_owned {
                    Some(p) => format!("{}{}", p, text),
                    None => text,
                };

                let current = entry_c.text();
                let new_text = if current.is_empty() {
                    formatted
                } else {
                    format!("{}\n{}", current, formatted)
                };
                entry_c.set_text(&new_text);
                status_c.set_visible(false);
            }
            Ok(Err(e)) => {
                status_c.set_text(&format!("❌ {}", e));
            }
            Err(e) => {
                status_c.set_text(&format!("❌ Transcription failed: {:?}", e));
            }
        }
    });
}

fn handle_audio_click(
    recorder: &Rc<RefCell<AudioRecorder>>,
    state: &Rc<RefCell<AppState>>,
    status_label: &Label,
    audio_btn: &Button,
    entry: &Entry,
) {
    if recorder.borrow().is_recording() {
        audio_btn.set_label("🎤 Record");

        let audio_path = match recorder.borrow_mut().stop() {
            Ok(path) => path,
            Err(e) => {
                status_label.set_text(&format!("❌ {}", e));
                status_label.set_visible(true);
                return;
            }
        };

        transcribe_and_insert(audio_path, state, status_label, entry, None);
    } else {
        match recorder.borrow_mut().start() {
            Ok(()) => {
                audio_btn.set_label("⏹️ Stop");
                status_label.set_text("🔴 Recording...");
                status_label.set_visible(true);
            }
            Err(e) => {
                status_label.set_text(&format!("❌ {}", e));
                status_label.set_visible(true);
            }
        }
    }
}

fn handle_listen_click(
    listener: &Rc<RefCell<ListenRecorder>>,
    state: &Rc<RefCell<AppState>>,
    status_label: &Label,
    listen_btn: &Button,
    entry: &Entry,
) {
    if listener.borrow().is_listening() {
        listen_btn.set_label("🎧 Listen");

        let audio_path = match listener.borrow_mut().stop() {
            Ok(path) => path,
            Err(e) => {
                status_label.set_text(&format!("❌ {}", e));
                status_label.set_visible(true);
                return;
            }
        };

        transcribe_and_insert(audio_path, state, status_label, entry, Some("🎧 [Heard]: "));
    } else {
        match listener.borrow_mut().start() {
            Ok(()) => {
                listen_btn.set_label("⏹️ Stop");
                status_label.set_text("🎧 Listening...");
                status_label.set_visible(true);
            }
            Err(e) => {
                status_label.set_text(&format!("❌ {}", e));
                status_label.set_visible(true);
            }
        }
    }
}

fn handle_screenshot_click(
    image: &Rc<RefCell<Option<ImageData>>>,
    status_label: &Label,
    attach_label: &Label,
) {
    status_label.set_text("📸 Capturing...");
    status_label.set_visible(true);

    // Take screenshot (stealth window is not captured by grim/gnome-screenshot)
    match capture::take_screenshot() {
        Ok(screenshot) => {
            let size_kb = screenshot.base64_data.len() / 1024;
            tracing::info!("Screenshot attached: {}KB", size_kb);

            *image.borrow_mut() = Some(ImageData {
                base64_data: screenshot.base64_data,
                mime_type: screenshot.mime_type,
            });

            attach_label.set_text(&format!("📎 Screenshot attached ({}KB)", size_kb));
            attach_label.set_visible(true);
            status_label.set_visible(false);
        }
        Err(e) => {
            status_label.set_text(&format!("❌ {}", e));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn send_message(
    entry: &Entry,
    state: &Rc<RefCell<AppState>>,
    chat_view: &TextView,
    status_label: &Label,
    send_btn: &Button,
    image: &Rc<RefCell<Option<ImageData>>>,
    attach_label: &Label,
) {
    let text = entry.text();
    if text.is_empty() {
        return;
    }

    let (api_key, model, system_prompt) = {
        let s = state.borrow();
        if s.settings.api_key.is_empty() {
            status_label.set_text("⚠️ Set API key in Settings");
            status_label.set_visible(true);
            return;
        }
        (
            s.settings.api_key.clone(),
            s.settings.model.clone(),
            s.prompts
                .first()
                .map(|p| p.content.clone())
                .unwrap_or_else(|| "You are a helpful AI assistant.".to_string()),
        )
    };

    // Take the attached image (if any)
    let attached = image.borrow_mut().take();
    attach_label.set_visible(false);

    // Add user message
    let has_image = attached.is_some();
    let user_content = if has_image {
        format!("{} [📷 Screenshot attached]", text)
    } else {
        text.to_string()
    };

    {
        state.borrow_mut().current_messages.push(Message {
            role: "user".to_string(),
            content: user_content,
        });
    }

    update_chat_display(chat_view, &state.borrow());
    entry.set_text("");

    send_btn.set_sensitive(false);
    status_label.set_text("⏳ Thinking...");
    status_label.set_visible(true);

    let messages: Vec<Message> = state.borrow().current_messages.clone();

    let state_c = state.clone();
    let chat_view_c = chat_view.clone();
    let status_c = status_label.clone();
    let send_btn_c = send_btn.clone();

    glib::spawn_future_local(async move {
        let result = gio::spawn_blocking(move || {
            let client = GroqClient::new(&api_key, &model);
            client
                .chat_with_image(&messages, &system_prompt, attached.as_ref())
                .map_err(|e| e.to_string())
        })
        .await;

        match result {
            Ok(Ok(response)) => {
                state_c.borrow_mut().current_messages.push(Message {
                    role: "assistant".to_string(),
                    content: response,
                });
                update_chat_display(&chat_view_c, &state_c.borrow());
                status_c.set_visible(false);
                send_btn_c.set_sensitive(true);
            }
            Ok(Err(e)) => {
                tracing::error!("API error: {}", e);
                status_c.set_text(&format!("❌ {}", e));
                send_btn_c.set_sensitive(true);
            }
            Err(e) => {
                tracing::error!("Task join error: {:?}", e);
                status_c.set_text("❌ Request failed");
                send_btn_c.set_sensitive(true);
            }
        }
    });
}

fn update_chat_display(chat_view: &TextView, state: &AppState) {
    let buffer = chat_view.buffer();

    if state.current_messages.is_empty() {
        buffer.set_text(
            "Welcome to ShyQuiet!\n\n💬 Type a message\n🎤 Record your voice\n🎧 Listen (system audio)\n📷 Attach screenshot",
        );
        return;
    }

    let text: String = state
        .current_messages
        .iter()
        .map(|m| {
            let prefix = if m.role == "user" {
                "👤 You"
            } else {
                "🤖 AI"
            };
            format!("{}: {}", prefix, m.content)
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    buffer.set_text(&text);

    if let Some(adj) = chat_view
        .parent()
        .and_then(|p| p.downcast::<ScrolledWindow>().ok())
        .map(|sw| sw.vadjustment())
    {
        adj.set_value(adj.upper());
    }
}
