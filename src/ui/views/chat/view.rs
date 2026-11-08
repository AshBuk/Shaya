// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Chat View UI construction
//!
//! Split into two parts for input passthrough:
//! - `build_content` - read-only content (passthrough area)
//! - `build_input` - interactive controls (control panel)

use super::{audio, live, markdown, messaging};
use crate::core::audio::{AudioRecorder, ListenRecorder};
use crate::core::state::{AppState, Conversation, ImageData, Role};
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
    transcript_label: Label,
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
    status_label.set_wrap(true);
    status_label.set_max_width_chars(40);
    status_label.add_css_class("dim-label");
    status_label.set_visible(false);
    if let Some(error) = &state.borrow().load_error {
        status_label.set_text(error);
        status_label.set_visible(true);
    }
    vbox.append(&status_label);

    let transcript_label = Label::new(None);
    transcript_label.set_halign(gtk4::Align::Start);
    transcript_label.set_selectable(true);
    transcript_label.set_wrap(true);
    transcript_label.set_max_width_chars(40);
    transcript_label.add_css_class("dim-label");
    transcript_label.set_visible(false);
    vbox.append(&transcript_label);

    // Attachment indicator
    let attachment_label = Label::new(None);
    attachment_label.set_halign(gtk4::Align::Start);
    attachment_label.add_css_class("accent");
    attachment_label.set_visible(false);
    vbox.append(&attachment_label);

    // Chat display (read-only, but allows text selection)
    // Use TextBuffer with markdown tag table for formatting
    let is_dark = state.borrow().settings.dark_mode;
    let tag_table = markdown::create_tag_table(is_dark);
    let buffer = TextBuffer::new(Some(&tag_table));

    // Update code tag colors when theme changes at runtime
    if let Some(display) = gtk4::gdk::Display::default() {
        let sm = adw::StyleManager::for_display(&display);
        let tt = tag_table.clone();
        sm.connect_dark_notify(move |sm| {
            markdown::update_theme(&tt, sm.is_dark());
        });
    }

    let chat_view = TextView::with_buffer(&buffer);
    chat_view.set_editable(false);
    chat_view.set_cursor_visible(false);
    chat_view.set_wrap_mode(WrapMode::WordChar);
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

    // Welcome page (shown when no messages)
    let welcome_page = adw::StatusPage::new();
    let logo_bytes = gtk4::glib::Bytes::from_static(include_bytes!("../../../../icons/shaya.png"));
    if let Ok(texture) = gtk4::gdk::Texture::from_bytes(&logo_bytes) {
        welcome_page.set_paintable(Some(&texture));
    }
    welcome_page.set_title("Shaya AI");
    welcome_page.set_description(Some("A fast and quiet AI assistant for deep work"));

    // Stack: toggle between welcome and chat
    let content_stack = gtk4::Stack::new();
    content_stack.set_vexpand(true);
    content_stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
    content_stack.set_transition_duration(150);
    content_stack.add_named(&welcome_page, Some("welcome"));
    content_stack.add_named(&scroll, Some("chat"));

    vbox.append(&content_stack);
    messaging::update_chat_display(&chat_view, &state.borrow());

    // Initialize shared state
    let shared = ChatSharedState {
        chat_view,
        status_label,
        transcript_label,
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
    let transcript_label = shared.transcript_label;
    let attachment_label = shared.attachment_label;
    let attached_image = shared.attached_image;
    let recorder = shared.recorder;
    let listener = shared.listener;

    // Input row: entry + send button
    let input_box = GtkBox::new(Orientation::Horizontal, 4);

    let entry = Entry::new();
    entry.set_hexpand(true);
    entry.set_placeholder_text(Some("Type message..."));

    let send_btn = Button::from_icon_name("go-up-symbolic");
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

    // Action buttons row
    let buttons_box = GtkBox::new(Orientation::Horizontal, 4);
    buttons_box.set_halign(gtk4::Align::Center);
    buttons_box.set_margin_top(4);

    let audio_btn = Button::from_icon_name("audio-input-microphone-symbolic");
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

    let listen_btn = Button::from_icon_name("audio-volume-high-symbolic");
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

    let live_btn = Button::from_icon_name("network-wireless-symbolic");
    live_btn.set_tooltip_text(Some("Start Live Assistant"));

    let reply_btn = Button::from_icon_name("mail-reply-sender-symbolic");
    reply_btn.set_tooltip_text(Some("Answer latest live transcript now"));
    reply_btn.set_sensitive(false);

    let live_runtime = Rc::new(RefCell::new(live::LiveRuntime::default()));
    let live_ctx = live::LiveContext {
        state: state.clone(),
        chat_view: chat_view.clone(),
        status_label: status_label.clone(),
        transcript_label: transcript_label.clone(),
        attached_image: attached_image.clone(),
        attach_label: attachment_label.clone(),
        audio_btn: audio_btn.clone(),
        listen_btn: listen_btn.clone(),
        live_btn: live_btn.clone(),
        reply_btn: reply_btn.clone(),
        recorder: recorder.clone(),
        listener: listener.clone(),
    };

    {
        let ctx = live_ctx.clone();
        let runtime = live_runtime.clone();
        live_btn.connect_clicked(move |_| {
            live::toggle(ctx.clone(), runtime.clone());
        });
    }

    {
        let ctx = live_ctx.clone();
        let runtime = live_runtime.clone();
        reply_btn.connect_clicked(move |_| {
            live::request_manual_answer(ctx.clone(), runtime.clone());
        });
    }

    let area_btn = Button::from_icon_name("edit-select-all-symbolic");
    area_btn.set_tooltip_text(Some("Select area to capture"));
    {
        let image_c = attached_image.clone();
        let status_c = status_label.clone();
        let attach_label_c = attachment_label.clone();

        area_btn.connect_clicked(move |btn| {
            messaging::handle_area_capture_click(btn, &image_c, &status_c, &attach_label_c);
        });
    }

    let screenshot_btn = Button::from_icon_name("camera-photo-symbolic");
    screenshot_btn.set_tooltip_text(Some("Capture full screen"));
    {
        let image_c = attached_image.clone();
        let status_c = status_label.clone();
        let attach_label_c = attachment_label.clone();

        screenshot_btn.connect_clicked(move |btn| {
            messaging::handle_screenshot_click(btn, &image_c, &status_c, &attach_label_c);
        });
    }

    let save_btn = Button::from_icon_name("document-save-symbolic");
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
                .find(|m| m.role == Role::User)
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

            // Create conversation and save
            let conv = Conversation {
                id,
                title,
                messages: state_mut.current_messages.clone(),
                created_at: now,
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

    let clear_btn = Button::from_icon_name("user-trash-symbolic");
    clear_btn.set_tooltip_text(Some("Clear chat"));
    {
        let state_c = state.clone();
        let chat_view_c = chat_view.clone();
        let image_c = attached_image.clone();
        let attach_label_c = attachment_label.clone();
        let entry_c = entry.clone();

        clear_btn.connect_clicked(move |_| {
            state_c.borrow_mut().replace_chat(Vec::new());
            *image_c.borrow_mut() = None;
            attach_label_c.set_visible(false);
            entry_c.set_text("");
            messaging::update_chat_display(&chat_view_c, &state_c.borrow());
        });
    }

    buttons_box.append(&audio_btn);
    buttons_box.append(&listen_btn);
    buttons_box.append(&live_btn);
    buttons_box.append(&reply_btn);
    buttons_box.append(&area_btn);
    buttons_box.append(&screenshot_btn);
    buttons_box.append(&save_btn);
    buttons_box.append(&clear_btn);
    vbox.append(&buttons_box);

    vbox
}
