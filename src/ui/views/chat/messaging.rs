// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Messaging handlers - chat messages and screenshots

use super::markdown;
use crate::core::api::groq::ImageData;
use crate::core::api::GroqClient;
use crate::core::capture;
use crate::core::state::{AppState, Message, Role};
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Button, Entry, Label, ScrolledWindow, TextView};
use std::cell::RefCell;
use std::rc::Rc;

/// Chat context containing all widgets needed for messaging
#[derive(Clone)]
pub struct ChatContext {
    pub entry: Entry,
    pub state: Rc<RefCell<AppState>>,
    pub chat_view: TextView,
    pub status_label: Label,
    pub send_btn: Button,
    pub image: Rc<RefCell<Option<ImageData>>>,
    pub attach_label: Label,
}

fn apply_captured_image(
    image: &Rc<RefCell<Option<ImageData>>>,
    attach_label: &Label,
    status_label: &Label,
    screenshot: capture::Screenshot,
    log_prefix: &str,
) {
    let size_kb = screenshot.base64_data.len() / 1024;
    tracing::info!("{}: {}KB", log_prefix, size_kb);

    *image.borrow_mut() = Some(ImageData {
        base64_data: screenshot.base64_data,
        mime_type: screenshot.mime_type,
    });

    attach_label.set_text(&format!("📎 Screenshot attached ({}KB)", size_kb));
    attach_label.set_visible(true);
    status_label.set_visible(false);
}

/// Handle area selection button click (slurp + grim)
pub fn handle_area_capture_click(
    image: &Rc<RefCell<Option<ImageData>>>,
    status_label: &Label,
    attach_label: &Label,
) {
    if capture::requires_async_portal() {
        handle_screenshot_click(image, status_label, attach_label);
        return;
    }

    status_label.set_text("🖱️ Select area...");
    status_label.set_visible(true);

    // Capture selected area (stealth-safe: slurp uses layer-shell Overlay)
    match capture::capture_area() {
        Ok(screenshot) => apply_captured_image(
            image,
            attach_label,
            status_label,
            screenshot,
            "Area screenshot attached",
        ),
        Err(e) => {
            status_label.set_text(&format!("❌ {}", e));
        }
    }
}

/// Handle full screenshot button click
pub fn handle_screenshot_click(
    image: &Rc<RefCell<Option<ImageData>>>,
    status_label: &Label,
    attach_label: &Label,
) {
    status_label.set_text("📸 Capturing...");
    status_label.set_visible(true);

    // GNOME Wayland portal flow is async and must run on the main loop.
    if capture::requires_async_portal() {
        let image_c = image.clone();
        let status_c = status_label.clone();
        let attach_c = attach_label.clone();
        glib::spawn_future_local(async move {
            match capture::take_screenshot_async().await {
                Ok(Some(screenshot)) => apply_captured_image(
                    &image_c,
                    &attach_c,
                    &status_c,
                    screenshot,
                    "Screenshot attached via portal",
                ),
                Ok(None) => status_c.set_visible(false),
                Err(e) => {
                    status_c.set_text(&format!("❌ {}", e));
                }
            }
        });
        return;
    }

    // Non-portal backends remain synchronous for now.
    match capture::take_screenshot() {
        Ok(screenshot) => apply_captured_image(
            image,
            attach_label,
            status_label,
            screenshot,
            "Screenshot attached",
        ),
        Err(e) => {
            status_label.set_text(&format!("❌ {}", e));
        }
    }
}

/// Send message to AI
pub fn send_message(ctx: &ChatContext) {
    let text = ctx.entry.text();
    if text.is_empty() {
        return;
    }

    let (api_key, model, system_prompt) = {
        let s = ctx.state.borrow();
        if s.settings.api_key.is_empty() {
            ctx.status_label.set_text("⚠️ Set API key in Settings");
            ctx.status_label.set_visible(true);
            return;
        }
        (
            s.settings.api_key.clone(),
            s.settings.model.clone(),
            s.active_prompt(),
        )
    };

    // Take the attached image (if any)
    let attached = ctx.image.borrow_mut().take();
    ctx.attach_label.set_visible(false);

    // Add user message
    let has_image = attached.is_some();
    let user_content = if has_image {
        format!("{} [📷 Screenshot attached]", text)
    } else {
        text.to_string()
    };

    ctx.state.borrow_mut().current_messages.push(Message {
        role: Role::User,
        content: user_content,
    });

    update_chat_display(&ctx.chat_view, &ctx.state.borrow());
    ctx.entry.set_text("");

    ctx.send_btn.set_sensitive(false);
    ctx.status_label.set_text("⏳ Thinking...");
    ctx.status_label.set_visible(true);

    let messages: Vec<Message> = ctx.state.borrow().current_messages.clone();
    let ctx_c = ctx.clone();

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
                ctx_c.state.borrow_mut().current_messages.push(Message {
                    role: Role::Assistant,
                    content: response,
                });
                update_chat_display(&ctx_c.chat_view, &ctx_c.state.borrow());
                ctx_c.status_label.set_visible(false);
                ctx_c.send_btn.set_sensitive(true);
            }
            Ok(Err(e)) => {
                tracing::error!("API error: {}", e);
                ctx_c.status_label.set_text(&format!("❌ {}", e));
                ctx_c.send_btn.set_sensitive(true);
            }
            Err(e) => {
                tracing::error!("Task join error: {:?}", e);
                ctx_c.status_label.set_text("❌ Request failed");
                ctx_c.send_btn.set_sensitive(true);
            }
        }
    });
}

/// Update chat display with current messages (with markdown rendering)
pub fn update_chat_display(chat_view: &TextView, state: &AppState) {
    let buffer = chat_view.buffer();

    if state.current_messages.is_empty() {
        buffer.set_text(
            "Shaya AI 🤫\n\n💬 Type a message\n🎤 Record your voice\n🎧 Listen (system audio)\n🖱️ Select area\n📷 Full screenshot",
        );
        return;
    }

    // Format messages with role prefixes
    let text: String = state
        .current_messages
        .iter()
        .map(|m| {
            let prefix = if m.role == Role::User {
                "👤 You"
            } else {
                "🤖 AI"
            };
            format!("**{}:**\n{}", prefix, m.content)
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    // Render with markdown formatting
    markdown::render(&buffer, &text);

    // Scroll to bottom
    if let Some(adj) = chat_view
        .parent()
        .and_then(|p| p.downcast::<ScrolledWindow>().ok())
        .map(|sw| sw.vadjustment())
    {
        adj.set_value(adj.upper());
    }
}
