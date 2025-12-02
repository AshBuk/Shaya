// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Chat View UI construction

use super::{audio, messaging};
use crate::core::api::groq::ImageData;
use crate::core::audio::{AudioRecorder, ListenRecorder};
use crate::core::state::AppState;
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
    messaging::update_chat_display(&chat_view, &state.borrow());

    // Input area
    let input_box = GtkBox::new(Orientation::Horizontal, 8);

    let entry = Entry::new();
    entry.set_hexpand(true);
    entry.set_placeholder_text(Some("Type your message..."));

    let send_btn = Button::with_label("Send");
    send_btn.add_css_class("suggested-action");

    // Send button handler
    {
        let entry_c = entry.clone();
        let state_c = state.clone();
        let chat_view_c = chat_view.clone();
        let status_c = status_label.clone();
        let send_btn_c = send_btn.clone();
        let image_c = attached_image.clone();
        let attach_label_c = attachment_label.clone();

        send_btn.connect_clicked(move |_| {
            messaging::send_message(
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

    // Entry activate handler (Enter key)
    {
        let state_c = state.clone();
        let chat_view_c = chat_view.clone();
        let status_c = status_label.clone();
        let send_btn_c = send_btn.clone();
        let image_c = attached_image.clone();
        let attach_label_c = attachment_label.clone();

        entry.connect_activate(move |entry| {
            messaging::send_message(
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
            audio::handle_audio_click(&recorder_c, &state_c, &status_c, &audio_btn_c, &entry_c);
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
            audio::handle_listen_click(&listener_c, &state_c, &status_c, &listen_btn_c, &entry_c);
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
            messaging::handle_screenshot_click(&image_c, &status_c, &attach_label_c);
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
            messaging::update_chat_display(&chat_view_c, &state_c.borrow());
        });
    }

    buttons_box.append(&audio_btn);
    buttons_box.append(&listen_btn);
    buttons_box.append(&screenshot_btn);
    buttons_box.append(&clear_btn);
    vbox.append(&buttons_box);

    vbox
}
