// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Messaging handlers - chat messages and screenshots

use super::markdown;
use crate::core::api::LlmClient;
use crate::core::capture;
use crate::core::state::{AppState, ImageData, Message, Role};
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Button, Entry, Label, TextView};
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

/// Wait for an external capture tool on a worker thread, so the UI and the
/// Live Assistant timers keep running while the user selects an area.
fn capture_in_background(
    button: &Button,
    image: &Rc<RefCell<Option<ImageData>>>,
    status_label: &Label,
    attach_label: &Label,
    capture: fn() -> Result<capture::Screenshot, String>,
    log_prefix: &'static str,
) {
    button.set_sensitive(false);
    let button = button.clone();
    let image = image.clone();
    let status_label = status_label.clone();
    let attach_label = attach_label.clone();

    glib::spawn_future_local(async move {
        let result = gio::spawn_blocking(capture)
            .await
            .unwrap_or_else(|_| Err("Capture task failed".to_string()));
        button.set_sensitive(true);
        match result {
            Ok(screenshot) => {
                apply_captured_image(&image, &attach_label, &status_label, screenshot, log_prefix)
            }
            Err(e) => status_label.set_text(&format!("❌ {}", e)),
        }
    });
}

/// Handle area selection button click (slurp + grim)
pub fn handle_area_capture_click(
    button: &Button,
    image: &Rc<RefCell<Option<ImageData>>>,
    status_label: &Label,
    attach_label: &Label,
) {
    if capture::requires_async_portal() {
        handle_screenshot_click(button, image, status_label, attach_label);
        return;
    }

    status_label.set_text("🖱️ Select area...");
    status_label.set_visible(true);

    capture_in_background(
        button,
        image,
        status_label,
        attach_label,
        capture::capture_area,
        "Area screenshot attached",
    );
}

/// Handle full screenshot button click
pub fn handle_screenshot_click(
    button: &Button,
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

    capture_in_background(
        button,
        image,
        status_label,
        attach_label,
        capture::take_screenshot,
        "Screenshot attached",
    );
}

/// Send message to AI
pub fn send_message(ctx: &ChatContext) {
    // Enter reaches this handler even while the send button is disabled.
    if !ctx.send_btn.is_sensitive() {
        return;
    }

    let text = ctx.entry.text();
    let has_attachment = ctx.image.borrow().is_some();
    if text.is_empty() && !has_attachment {
        return;
    }

    let (route, system_prompt) = {
        let s = ctx.state.borrow();
        let has_images = has_attachment || s.current_messages.iter().any(|m| m.image.is_some());
        (s.settings.chat_route(has_images), s.active_prompt())
    };
    let image_notice = route.image_notice();
    if route.api_key.is_empty() {
        let hint = image_notice.map_or(String::new(), |notice| format!(", {notice}"));
        ctx.status_label
            .set_text(&format!("⚠️ Set API key in Settings{hint}"));
        ctx.status_label.set_visible(true);
        return;
    }

    // Take the attached image (if any)
    let attached = ctx.image.borrow_mut().take();
    let unsent_image = attached.clone();
    ctx.attach_label.set_visible(false);

    // Add user message with image stored inline
    let has_image = attached.is_some();
    let user_content = match (text.is_empty(), has_image) {
        (true, true) => "📷 Screenshot".to_string(),
        (false, true) => format!("{} [📷 Screenshot attached]", text),
        _ => text.to_string(),
    };

    ctx.state.borrow_mut().current_messages.push(Message {
        role: Role::User,
        content: user_content,
        image: attached,
    });

    update_chat_display(&ctx.chat_view, &ctx.state.borrow());
    ctx.entry.set_text("");

    ctx.send_btn.set_sensitive(false);
    ctx.status_label.set_text(&match image_notice {
        Some(notice) => format!("⏳ Thinking, {notice}..."),
        None => "⏳ Thinking...".to_string(),
    });
    ctx.status_label.set_visible(true);

    let (messages, generation) = {
        let state = ctx.state.borrow();
        (state.current_messages.clone(), state.chat_generation)
    };
    let user_message_index = messages.len() - 1;
    let ctx_c = ctx.clone();

    glib::spawn_future_local(async move {
        let result = gio::spawn_blocking(move || {
            let client = LlmClient::new(route.provider, &route.api_key, &route.model);
            client
                .chat(&messages, &system_prompt)
                .map_err(|e| e.to_string())
        })
        .await;

        ctx_c.send_btn.set_sensitive(true);
        // The chat was cleared or replaced while waiting: this result belongs to no open chat.
        if ctx_c.state.borrow().chat_generation != generation {
            ctx_c.status_label.set_visible(false);
            return;
        }

        let failure = match result {
            Ok(Ok(response)) => {
                ctx_c.state.borrow_mut().current_messages.push(Message {
                    role: Role::Assistant,
                    content: response,
                    image: None,
                });
                update_chat_display(&ctx_c.chat_view, &ctx_c.state.borrow());
                ctx_c.status_label.set_visible(false);
                return;
            }
            Ok(Err(e)) => e,
            Err(e) => {
                tracing::error!("Task join error: {:?}", e);
                "Request failed".to_string()
            }
        };

        tracing::error!("API error: {}", failure);
        ctx_c.status_label.set_text(&format!("❌ {}", failure));
        restore_unsent_message(&ctx_c, user_message_index, &text, unsent_image);
    });
}

/// Take a failed message out of the chat and back into the input. If it stayed,
/// every later request would resend it, and a rejected image would block the chat.
fn restore_unsent_message(ctx: &ChatContext, index: usize, text: &str, image: Option<ImageData>) {
    {
        let mut state = ctx.state.borrow_mut();
        if index < state.current_messages.len() {
            state.current_messages.remove(index);
        }
    }
    update_chat_display(&ctx.chat_view, &ctx.state.borrow());

    // The input stays editable during a request and may hold a newer draft.
    let draft = ctx.entry.text();
    let merged = with_unsent_text(text, &draft);
    if merged != draft {
        ctx.entry.set_text(&merged);
        ctx.entry.set_position(-1);
    }

    if image.is_some() {
        if ctx.image.borrow().is_none() {
            *ctx.image.borrow_mut() = image;
            ctx.attach_label.set_visible(true);
        } else {
            // One attachment slot, and the user chose the newer screenshot last.
            let failure = ctx.status_label.text();
            ctx.status_label.set_text(&format!(
                "{failure} (unsent screenshot replaced by a newer one)"
            ));
        }
    }
}

fn with_unsent_text(unsent: &str, draft: &str) -> String {
    match (unsent.is_empty(), draft.is_empty()) {
        (true, _) => draft.to_string(),
        (_, true) => unsent.to_string(),
        _ => format!("{unsent} {draft}"),
    }
}

/// Toggle between welcome page and chat content via parent Stack.
fn toggle_chat_stack(chat_view: &TextView, show_chat: bool) {
    // chat_view -> ScrolledWindow -> Stack
    if let Some(scroll) = chat_view.parent() {
        if let Some(stack) = scroll
            .parent()
            .and_then(|p| p.downcast::<gtk4::Stack>().ok())
        {
            stack.set_visible_child_name(if show_chat { "chat" } else { "welcome" });
        }
    }
}

/// Update chat display with current messages (with markdown rendering)
pub fn update_chat_display(chat_view: &TextView, state: &AppState) {
    let buffer = chat_view.buffer();
    let has_messages = !state.current_messages.is_empty();

    toggle_chat_stack(chat_view, has_messages);

    if !has_messages {
        buffer.set_text("");
        return;
    }

    // Render messages one by one, tracking the last message offset
    buffer.set_text("");
    let mut last_offset: i32 = 0;

    for (idx, m) in state.current_messages.iter().enumerate() {
        let prefix = if m.role == Role::User {
            "👤 You"
        } else {
            "🤖 AI"
        };
        let block = format!("**{}:**\n{}", prefix, m.content);

        if idx > 0 {
            let mut end = buffer.end_iter();
            buffer.insert(&mut end, "\n\n");
        }

        last_offset = markdown::render_append(&buffer, &block);
    }

    // Scroll to the start of the last message
    let scroll_offset = last_offset;
    let chat_view_clone = chat_view.clone();
    glib::idle_add_local_once(move || {
        let buffer = chat_view_clone.buffer();
        let iter = buffer.iter_at_offset(scroll_offset);
        chat_view_clone.scroll_to_iter(&mut iter.clone(), 0.0, true, 0.0, 0.0);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsent_text_survives_a_newer_draft() {
        assert_eq!(with_unsent_text("first", ""), "first");
        assert_eq!(with_unsent_text("first", "second"), "first second");
        assert_eq!(with_unsent_text("", "second"), "second");
    }
}
