// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Messaging handlers - chat messages and screenshots

use super::status::StatusBar;
use super::task::{self, ChatScope};
use super::transcript::{image_texture, Transcript, SCREENSHOT_NOTE, SCREENSHOT_ONLY};
use crate::core::api::LlmClient;
use crate::core::capture;
use crate::core::state::{AppState, ImageData, Message, Role};
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, MenuButton, Orientation, Picture, TextBuffer};
use std::cell::RefCell;
use std::rc::Rc;

/// Chat context containing all widgets needed for messaging
#[derive(Clone)]
pub struct ChatContext {
    pub input: TextBuffer,
    pub state: Rc<RefCell<AppState>>,
    pub transcript: Transcript,
    pub status: StatusBar,
    pub send_btn: Button,
    pub attachment: Attachment,
}

/// The screenshot waiting to be sent, with its preview in the composer.
#[derive(Clone)]
pub struct Attachment {
    pub image: Rc<RefCell<Option<ImageData>>>,
    root: GtkBox,
    picture: Picture,
    label: Label,
}

impl Attachment {
    pub fn new() -> Self {
        let picture = Picture::new();
        picture.add_css_class("attachment-thumb");
        picture.set_overflow(gtk4::Overflow::Hidden);
        picture.set_size_request(48, 32);

        let label = Label::new(None);
        label.add_css_class("attachment-label");

        let remove = Button::from_icon_name("window-close-symbolic");
        remove.add_css_class("flat");
        remove.add_css_class("small-action");
        remove.set_tooltip_text(Some("Remove screenshot"));

        let root = GtkBox::new(Orientation::Horizontal, 8);
        root.add_css_class("attachment-chip");
        root.set_halign(Align::Start);
        root.set_visible(false);
        root.append(&picture);
        root.append(&label);
        root.append(&remove);

        let attachment = Self {
            image: Rc::default(),
            root,
            picture,
            label,
        };
        let attachment_c = attachment.clone();
        remove.connect_clicked(move |_| attachment_c.clear());
        attachment
    }

    pub fn widget(&self) -> &GtkBox {
        &self.root
    }

    pub fn set(&self, image: ImageData) {
        *self.image.borrow_mut() = Some(image);
        self.sync();
    }

    pub fn take(&self) -> Option<ImageData> {
        let image = self.image.borrow_mut().take();
        self.sync();
        image
    }

    pub fn clear(&self) {
        self.take();
    }

    /// Show what `image` holds.
    pub fn sync(&self) {
        let image = self.image.borrow();
        self.root.set_visible(image.is_some());
        if let Some(image) = image.as_ref() {
            let texture = image_texture(image);
            self.picture.set_paintable(texture.as_ref());
            self.label.set_text(&format!(
                "Screenshot · {} KB",
                image.base64_data.len() / 1024
            ));
        } else {
            // A hidden picture still holds the full-size texture.
            self.picture.set_paintable(None::<&gtk4::gdk::Texture>);
        }
    }
}

/// Text of the input. No hidden characters, so it can go to the model as is.
pub fn input_text(input: &TextBuffer) -> String {
    let (start, end) = input.bounds();
    input.text(&start, &end, false).to_string()
}

fn apply_captured_image(
    attachment: &Attachment,
    status: &StatusBar,
    screenshot: capture::Screenshot,
    log_prefix: &str,
) {
    tracing::info!("{}: {}KB", log_prefix, screenshot.base64_data.len() / 1024);
    attachment.set(ImageData {
        base64_data: screenshot.base64_data,
        mime_type: screenshot.mime_type,
    });
    status.clear();
}

/// Wait for an external capture tool on a worker thread, so the UI and the
/// Live Assistant timers keep running while the user selects an area.
fn capture_in_background(
    button: &MenuButton,
    attachment: &Attachment,
    status: &StatusBar,
    capture: fn() -> anyhow::Result<capture::Screenshot>,
    log_prefix: &'static str,
) {
    button.set_sensitive(false);
    let button = button.clone();
    let attachment = attachment.clone();
    let status = status.clone();

    glib::spawn_future_local(async move {
        let result = gio::spawn_blocking(capture)
            .await
            .unwrap_or_else(|_| Err(anyhow::anyhow!("Capture task failed")));
        button.set_sensitive(true);
        match result {
            Ok(screenshot) => apply_captured_image(&attachment, &status, screenshot, log_prefix),
            Err(e) => status.error(&format!("{e:#}")),
        }
    });
}

/// Handle area selection (slurp + grim)
pub fn handle_area_capture_click(button: &MenuButton, attachment: &Attachment, status: &StatusBar) {
    if capture::requires_async_portal() {
        handle_screenshot_click(button, attachment, status);
        return;
    }

    status.busy("Select an area…");

    capture_in_background(
        button,
        attachment,
        status,
        capture::capture_area,
        "Area screenshot attached",
    );
}

/// Handle full screenshot
pub fn handle_screenshot_click(button: &MenuButton, attachment: &Attachment, status: &StatusBar) {
    status.busy("Capturing…");

    // GNOME Wayland portal flow is async and must run on the main loop.
    if capture::requires_async_portal() {
        button.set_sensitive(false);
        let button = button.clone();
        let attachment = attachment.clone();
        let status = status.clone();
        glib::spawn_future_local(async move {
            let result = capture::take_screenshot_async().await;
            button.set_sensitive(true);
            match result {
                Ok(Some(screenshot)) => apply_captured_image(
                    &attachment,
                    &status,
                    screenshot,
                    "Screenshot attached via portal",
                ),
                Ok(None) => status.clear(),
                Err(e) => status.error(&format!("{e:#}")),
            }
        });
        return;
    }

    capture_in_background(
        button,
        attachment,
        status,
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

    let draft = input_text(&ctx.input);
    let text = draft.trim().to_string();
    let has_attachment = ctx.attachment.image.borrow().is_some();
    if text.is_empty() && !has_attachment {
        return;
    }

    let (route, system_prompt) = {
        let s = ctx.state.borrow();
        let has_images = has_attachment || s.current_messages.iter().any(|m| m.image.is_some());
        (s.settings.chat_route(has_images), s.active_prompt())
    };
    let image_notice = route.image_notice();
    if let Some(missing) = route.missing() {
        let hint = image_notice.map_or(String::new(), |notice| format!(", {notice}"));
        ctx.status.error(&format!("{missing}{hint}"));
        return;
    }

    // Take the attached image (if any)
    let attached = ctx.attachment.take();
    let unsent_image = attached.clone();

    // Add user message with image stored inline
    let has_image = attached.is_some();
    let user_content = match (text.is_empty(), has_image) {
        (true, true) => SCREENSHOT_ONLY.to_string(),
        (false, true) => format!("{text}{SCREENSHOT_NOTE}"),
        _ => text.clone(),
    };

    ctx.state.borrow_mut().current_messages.push(Message {
        role: Role::User,
        content: user_content,
        image: attached,
    });

    ctx.transcript.update(&ctx.state.borrow());
    ctx.input.set_text("");

    ctx.send_btn.set_sensitive(false);
    ctx.status.busy(&match image_notice {
        Some(notice) => format!("Thinking, {notice}…"),
        None => "Thinking…".to_string(),
    });

    let (messages, scope) = {
        let state = ctx.state.borrow();
        (state.current_messages.clone(), ChatScope::current(&state))
    };
    let user_message_index = messages.len() - 1;
    let ctx_c = ctx.clone();

    glib::spawn_future_local(async move {
        let result = task::run(&ctx_c.state, scope, move || {
            LlmClient::for_route(&route).chat(&messages, &system_prompt)
        })
        .await;

        ctx_c.send_btn.set_sensitive(true);
        let failure = match result {
            // The chat was cleared or replaced while waiting: the reply belongs to no open chat.
            None => {
                ctx_c.status.clear();
                return;
            }
            Some(Ok(response)) => {
                ctx_c.state.borrow_mut().current_messages.push(Message {
                    role: Role::Assistant,
                    content: response,
                    image: None,
                });
                ctx_c.transcript.update(&ctx_c.state.borrow());
                ctx_c.status.clear();
                return;
            }
            Some(Err(e)) => format!("{e:#}"),
        };

        tracing::error!("API error: {}", failure);
        restore_unsent_message(&ctx_c, user_message_index, &text, unsent_image, &failure);
    });
}

/// Take a failed message out of the chat and back into the input. If it stayed,
/// every later request would resend it, and a rejected image would block the chat.
fn restore_unsent_message(
    ctx: &ChatContext,
    index: usize,
    text: &str,
    image: Option<ImageData>,
    failure: &str,
) {
    {
        let mut state = ctx.state.borrow_mut();
        if index < state.current_messages.len() {
            state.current_messages.remove(index);
        }
    }
    ctx.transcript.update(&ctx.state.borrow());
    ctx.status.error(failure);

    // The input stays editable during a request and may hold a newer draft.
    let draft = input_text(&ctx.input);
    let merged = with_unsent_text(text, &draft);
    if merged != draft {
        ctx.input.set_text(&merged);
        ctx.input.place_cursor(&ctx.input.end_iter());
    }

    if let Some(image) = image {
        if ctx.attachment.image.borrow().is_none() {
            ctx.attachment.set(image);
        } else {
            // One attachment slot, and the user chose the newer screenshot last.
            ctx.status.error(&format!(
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
