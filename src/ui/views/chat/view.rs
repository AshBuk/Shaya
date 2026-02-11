// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Chat View UI construction
//!
//! Split into two parts for input passthrough:
//! - `build_content` - read-only content (passthrough area)
//! - `build_input` - interactive controls (control panel)

use super::{audio, markdown, messaging};
use crate::core::api::groq::ImageData;
use crate::core::audio::{AudioRecorder, ListenRecorder};
use crate::core::state::{AppState, Conversation};
use chrono::Utc;
use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, Entry, Label, Orientation, PolicyType, ScrolledWindow, TextBuffer,
    TextView, WrapMode,
};
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;

/// Shared state for chat (passed explicitly between content and input builders)
#[derive(Clone)]
pub(crate) struct ChatSharedState {
    pub(crate) chat_view: TextView,
    status_label: Label,
    attachment_label: Label,
    attached_image: Rc<RefCell<Option<ImageData>>>,
    recorder: Rc<RefCell<AudioRecorder>>,
    listener: Rc<RefCell<ListenRecorder>>,
}

/// Build read-only content area (passthrough - clicks pass through)
/// Returns the widget and shared state needed by `build_input` and `refresh_display`.
pub fn build_content(state: Rc<RefCell<AppState>>) -> (GtkBox, ChatSharedState) {
    let vbox = GtkBox::new(Orientation::Vertical, 4);
    vbox.set_margin_top(4);
    vbox.set_margin_start(8);
    vbox.set_margin_end(8);
    vbox.set_vexpand(true);

    // Status label (shows recording/transcribing state)
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

    // Chat display (read-only, but allows text selection)
    // Use TextBuffer with markdown tag table for formatting
    let tag_table = markdown::create_tag_table();
    let buffer = TextBuffer::new(Some(&tag_table));
    let chat_view = TextView::with_buffer(&buffer);
    chat_view.set_editable(false);
    chat_view.set_cursor_visible(false);
    chat_view.set_wrap_mode(WrapMode::Word);
    chat_view.set_vexpand(true);
    chat_view.set_top_margin(8);
    chat_view.set_bottom_margin(8);
    chat_view.set_left_margin(8);
    chat_view.set_right_margin(8);
    // Compact line spacing (code blocks need this)
    chat_view.set_pixels_above_lines(0);
    chat_view.set_pixels_below_lines(0);
    chat_view.set_pixels_inside_wrap(0);

    // ScrolledWindow handles mouse scroll events
    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .vexpand(true)
        .child(&chat_view)
        .build();

    vbox.append(&scroll);
    messaging::update_chat_display(&chat_view, &state.borrow());

    // Initialize shared state
    let shared = ChatSharedState {
        chat_view,
        status_label,
        attachment_label,
        attached_image: Rc::new(RefCell::new(None)),
        recorder: Rc::new(RefCell::new(AudioRecorder::new())),
        listener: Rc::new(RefCell::new(ListenRecorder::new())),
    };

    (vbox, shared)
}

/// Refresh chat display from external call (e.g., after loading from history)
pub fn refresh_display(chat_view: &TextView, state: &AppState) {
    messaging::update_chat_display(chat_view, state);
}

/// Build interactive input area (control panel - receives input)
pub fn build_input(
    state: Rc<RefCell<AppState>>,
    _stack: &adw::ViewStack,
    shared: ChatSharedState,
) -> GtkBox {
    let vbox = GtkBox::new(Orientation::Vertical, 4);

    let chat_view = shared.chat_view;
    let status_label = shared.status_label;
    let attachment_label = shared.attachment_label;
    let attached_image = shared.attached_image;
    let recorder = shared.recorder;
    let listener = shared.listener;

    // Input row: entry + send button
    let input_box = GtkBox::new(Orientation::Horizontal, 4);

    let entry = Entry::new();
    entry.set_hexpand(true);
    entry.set_placeholder_text(Some("Type message..."));

    let send_btn = Button::with_label("↑");
    send_btn.add_css_class("suggested-action");
    send_btn.set_tooltip_text(Some("Send message"));

    // Chat context for send handlers
    let chat_ctx = messaging::ChatContext {
        entry: entry.clone(),
        state: state.clone(),
        chat_view: chat_view.clone(),
        status_label: status_label.clone(),
        send_btn: send_btn.clone(),
        image: attached_image.clone(),
        attach_label: attachment_label.clone(),
    };

    // Send handler
    {
        let ctx = chat_ctx.clone();
        send_btn.connect_clicked(move |_| {
            messaging::send_message(&ctx);
        });
    }

    // Enter key handler
    {
        let ctx = chat_ctx.clone();
        entry.connect_activate(move |_| {
            messaging::send_message(&ctx);
        });
    }

    input_box.append(&entry);
    input_box.append(&send_btn);
    vbox.append(&input_box);

    // Action buttons row (compact)
    let buttons_box = GtkBox::new(Orientation::Horizontal, 4);
    buttons_box.set_halign(gtk4::Align::Center);
    buttons_box.set_margin_top(4);

    // 🎤 Record
    let audio_btn = Button::with_label("🎤");
    audio_btn.set_tooltip_text(Some("Record and transcribe"));
    {
        let recorder_c = recorder.clone();
        let state_c = state.clone();
        let status_c = status_label.clone();
        let audio_btn_c = audio_btn.clone();
        let entry_c = entry.clone();

        audio_btn.connect_clicked(move |_| {
            audio::handle_audio_click(&recorder_c, &state_c, &status_c, &audio_btn_c, &entry_c);
        });
    }

    // 🎧 Listen
    let listen_btn = Button::with_label("🎧");
    listen_btn.set_tooltip_text(Some("Capture system audio"));
    {
        let listener_c = listener.clone();
        let state_c = state.clone();
        let status_c = status_label.clone();
        let listen_btn_c = listen_btn.clone();
        let entry_c = entry.clone();

        listen_btn.connect_clicked(move |_| {
            audio::handle_listen_click(&listener_c, &state_c, &status_c, &listen_btn_c, &entry_c);
        });
    }

    // 🖱️ Select area
    let area_btn = Button::with_label("🖱️");
    area_btn.set_tooltip_text(Some("Select area to capture"));
    {
        let image_c = attached_image.clone();
        let status_c = status_label.clone();
        let attach_label_c = attachment_label.clone();

        area_btn.connect_clicked(move |_| {
            messaging::handle_area_capture_click(&image_c, &status_c, &attach_label_c);
        });
    }

    // 📷 Full screenshot
    let screenshot_btn = Button::with_label("📷");
    screenshot_btn.set_tooltip_text(Some("Capture full screen"));
    {
        let image_c = attached_image.clone();
        let status_c = status_label.clone();
        let attach_label_c = attachment_label.clone();

        screenshot_btn.connect_clicked(move |_| {
            messaging::handle_screenshot_click(&image_c, &status_c, &attach_label_c);
        });
    }

    // 💾 Save to history
    let save_btn = Button::with_label("💾");
    save_btn.set_tooltip_text(Some("Save to history"));
    {
        let state_c = state.clone();
        let status_c = status_label.clone();

        save_btn.connect_clicked(move |_| {
            let mut state_mut = state_c.borrow_mut();
            if state_mut.current_messages.is_empty() {
                status_c.set_text("⚠️ Nothing to save");
                status_c.set_visible(true);
                return;
            }

            // Generate title from first user message (max 50 chars)
            let title = state_mut
                .current_messages
                .iter()
                .find(|m| m.role == "user")
                .map(|m| {
                    let trimmed: String = m.content.chars().take(50).collect();
                    if m.content.chars().count() > 50 {
                        format!("{}...", trimmed)
                    } else {
                        trimmed
                    }
                })
                .unwrap_or_else(|| "Untitled".to_string());

            // Generate unique ID and timestamp
            let now = Utc::now();
            let id = now.timestamp_millis().to_string();
            let created_at = now.format("%Y-%m-%d %H:%M").to_string();

            // Create conversation and save
            let conv = Conversation {
                id,
                title,
                messages: state_mut.current_messages.clone(),
                created_at,
            };

            state_mut.history.push(conv);
            if let Err(e) = state_mut.save() {
                tracing::error!("Failed to save history: {}", e);
                status_c.set_text("❌ Save failed");
                status_c.set_visible(true);
            } else {
                status_c.set_text("✅ Saved to history");
                status_c.set_visible(true);
            }
        });
    }

    // 🗑️ Clear (silent, no confirmation)
    let clear_btn = Button::with_label("🗑️");
    clear_btn.set_tooltip_text(Some("Clear chat"));
    {
        let state_c = state.clone();
        let chat_view_c = chat_view.clone();
        let image_c = attached_image.clone();
        let attach_label_c = attachment_label.clone();
        let entry_c = entry.clone();

        clear_btn.connect_clicked(move |_| {
            state_c.borrow_mut().current_messages.clear();
            *image_c.borrow_mut() = None;
            attach_label_c.set_visible(false);
            entry_c.set_text("");
            messaging::update_chat_display(&chat_view_c, &state_c.borrow());
        });
    }

    buttons_box.append(&audio_btn);
    buttons_box.append(&listen_btn);
    buttons_box.append(&area_btn);
    buttons_box.append(&screenshot_btn);
    buttons_box.append(&save_btn);
    buttons_box.append(&clear_btn);
    vbox.append(&buttons_box);

    vbox
}
