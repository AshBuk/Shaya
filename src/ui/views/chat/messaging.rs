// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Messaging handlers - chat messages and screenshots

use crate::core::api::groq::ImageData;
use crate::core::api::GroqClient;
use crate::core::capture;
use crate::core::state::{AppState, Message};
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Button, Entry, Label, ScrolledWindow, TextView};
use std::cell::RefCell;
use std::rc::Rc;

/// Handle screenshot button click
pub fn handle_screenshot_click(
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

/// Send message to AI
#[allow(clippy::too_many_arguments)]
pub fn send_message(
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

/// Update chat display with current messages
pub fn update_chat_display(chat_view: &TextView, state: &AppState) {
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
