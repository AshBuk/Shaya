// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Conversation display: one widget per message, with separate surfaces for code.

use super::markdown::{self, Block, MarkdownView};
use crate::core::state::{AppState, ImageData, Message, Role};
use base64::Engine;
use gtk4::prelude::*;
use gtk4::{
    gdk, glib, Align, Box as GtkBox, Button, Label, Orientation, Picture, PolicyType,
    ScrolledWindow, Stack, TextBuffer, TextTagTable, TextView, WrapMode,
};
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

pub(super) const SCREENSHOT_ONLY: &str = "📷 Screenshot";
pub(super) const SCREENSHOT_NOTE: &str = " [📷 Screenshot attached]";

/// Frames to keep following a message while its text views settle on a height.
const SCROLL_SETTLE_FRAMES: u8 = 20;

#[derive(Clone)]
pub(crate) struct Transcript {
    stack: Stack,
    scroll: ScrolledWindow,
    list: GtkBox,
    tags: TextTagTable,
    /// Chat generation and message count on screen
    rendered: Rc<Cell<(u64, usize)>>,
}

impl Transcript {
    pub fn new(empty: &impl IsA<gtk4::Widget>, tags: TextTagTable) -> Self {
        let list = GtkBox::new(Orientation::Vertical, 20);
        list.add_css_class("message-list");
        list.set_valign(Align::Start);

        let scroll = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .vexpand(true)
            .child(&list)
            .build();

        // Selecting text in a long message must not make the view jump to its start.
        if let Some(viewport) = scroll.child() {
            if viewport.find_property("scroll-to-focus").is_some() {
                viewport.set_property("scroll-to-focus", false);
            }
        }

        let stack = Stack::new();
        stack.set_vexpand(true);
        stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
        stack.set_transition_duration(150);
        stack.add_named(empty, Some("empty"));
        stack.add_named(&scroll, Some("chat"));

        Self {
            stack,
            scroll,
            list,
            tags,
            rendered: Rc::default(),
        }
    }

    pub fn widget(&self) -> &Stack {
        &self.stack
    }

    /// Show the current messages. Appends new ones, and rebuilds only if the chat changed.
    pub fn update(&self, state: &AppState) {
        let messages = &state.current_messages;
        let (generation, count) = self.rendered.get();
        let first_new = if generation != state.chat_generation || count > messages.len() {
            while let Some(child) = self.list.first_child() {
                self.list.remove(&child);
            }
            0
        } else {
            count
        };

        let mut last = None;
        for message in &messages[first_new..] {
            let widget = self.message_widget(message);
            self.list.append(&widget);
            last = Some(widget);
        }
        self.rendered.set((state.chat_generation, messages.len()));

        self.stack
            .set_visible_child_name(if messages.is_empty() { "empty" } else { "chat" });
        if let Some(widget) = last {
            self.scroll_to(widget);
        }
    }

    /// Bring a message to the top of the view, once it has a position.
    fn scroll_to(&self, target: GtkBox) {
        let list = self.list.clone();
        let frames = Cell::new(0u8);
        self.scroll.add_tick_callback(move |scroll, _| {
            let Some((_, y)) = target.translate_coordinates(&list, 0.0, 0.0) else {
                return glib::ControlFlow::Break;
            };
            scroll.vadjustment().set_value(y);
            frames.set(frames.get() + 1);
            if frames.get() < SCROLL_SETTLE_FRAMES {
                glib::ControlFlow::Continue
            } else {
                glib::ControlFlow::Break
            }
        });
    }

    fn message_widget(&self, message: &Message) -> GtkBox {
        if message.role == Role::User {
            user_message(message)
        } else {
            self.assistant_message(&message.content)
        }
    }

    fn assistant_message(&self, content: &str) -> GtkBox {
        let root = GtkBox::new(Orientation::Vertical, 8);
        root.add_css_class("assistant-message");

        for block in markdown::split_blocks(content) {
            match block {
                Block::Text(text) => root.append(&self.text_view(&text)),
                Block::Code { lang, code } => root.append(&code_card(&lang, &code)),
            }
        }

        let copy = copy_button(content, "Copy message");
        copy.add_css_class("message-actions");
        copy.set_halign(Align::Start);
        root.append(&copy);
        root
    }

    fn text_view(&self, text: &str) -> TextView {
        let buffer = TextBuffer::new(Some(&self.tags));
        let view = TextView::builder()
            .buffer(&buffer)
            .editable(false)
            .cursor_visible(false)
            .wrap_mode(WrapMode::WordChar)
            .pixels_above_lines(2)
            .pixels_below_lines(2)
            .pixels_inside_wrap(3)
            .build();
        view.add_css_class("message-text");
        MarkdownView::new(&view).render(text);
        view
    }
}

fn user_message(message: &Message) -> GtkBox {
    let root = GtkBox::new(Orientation::Vertical, 6);
    root.add_css_class("user-bubble");
    root.set_halign(Align::End);
    root.set_margin_start(32);

    if let Some(texture) = message.image.as_ref().and_then(image_texture) {
        let picture = Picture::for_paintable(&texture);
        picture.add_css_class("bubble-image");
        picture.set_overflow(gtk4::Overflow::Hidden);
        picture.set_size_request(180, 110);
        root.append(&picture);
    }

    let text = display_text(&message.content, message.image.is_some());
    if !text.is_empty() {
        let label = Label::new(Some(text));
        label.set_xalign(0.0);
        label.set_wrap(true);
        label.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
        label.set_selectable(true);
        root.append(&label);
    }
    root
}

/// The thumbnail replaces the screenshot note that goes to the model and to history.
fn display_text(content: &str, has_image: bool) -> &str {
    if !has_image {
        return content;
    }
    if content == SCREENSHOT_ONLY {
        return "";
    }
    content.strip_suffix(SCREENSHOT_NOTE).unwrap_or(content)
}

fn code_card(lang: &str, code: &str) -> GtkBox {
    let name = Label::new(Some(if lang.is_empty() { "code" } else { lang }));
    name.add_css_class("code-lang");
    name.set_hexpand(true);
    name.set_xalign(0.0);

    let header = GtkBox::new(Orientation::Horizontal, 4);
    header.add_css_class("code-card-header");
    header.append(&name);
    header.append(&copy_button(code, "Copy code"));

    let text = Label::new(Some(code));
    text.add_css_class("code-text");
    text.set_xalign(0.0);
    text.set_yalign(0.0);
    text.set_selectable(true);

    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Automatic)
        .vscrollbar_policy(PolicyType::Never)
        .child(&text)
        .build();

    let card = GtkBox::new(Orientation::Vertical, 0);
    card.add_css_class("code-card");
    card.set_overflow(gtk4::Overflow::Hidden);
    card.append(&header);
    card.append(&scroll);
    card
}

fn copy_button(text: &str, tooltip: &str) -> Button {
    let button = Button::from_icon_name("edit-copy-symbolic");
    button.add_css_class("flat");
    button.add_css_class("small-action");
    button.set_tooltip_text(Some(tooltip));

    let text = text.to_string();
    button.connect_clicked(move |button| {
        button.clipboard().set_text(&text);
        button.set_icon_name("object-select-symbolic");
        let button = button.clone();
        glib::timeout_add_local_once(Duration::from_millis(1200), move || {
            button.set_icon_name("edit-copy-symbolic");
        });
    });
    button
}

/// Decode an attached image for display.
pub(super) fn image_texture(image: &ImageData) -> Option<gdk::Texture> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&image.base64_data)
        .ok()?;
    gdk::Texture::from_bytes(&glib::Bytes::from_owned(bytes)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumbnail_replaces_only_the_screenshot_note() {
        assert_eq!(display_text(SCREENSHOT_ONLY, true), "");
        assert_eq!(
            display_text(&format!("what is this{SCREENSHOT_NOTE}"), true),
            "what is this"
        );
        // History does not keep images, so the note stays the only trace of one.
        assert_eq!(display_text(SCREENSHOT_ONLY, false), SCREENSHOT_ONLY);
    }
}
