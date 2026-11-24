// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Chat View UI construction
//!
//! Split into two parts:
//! - `build_content` - transcript and status, a page of the view stack
//! - `build_input` - composer and controls, in the panel below the stack

use super::messaging::Attachment;
use super::status::StatusBar;
use super::transcript::Transcript;
use super::{audio, live, markdown, messaging};
use crate::core::audio::{AudioRecorder, ListenRecorder};
use crate::core::models::display_name;
use crate::core::state::{AppState, Conversation, Role, Settings};
use chrono::Utc;
use gtk4::gio;
use gtk4::prelude::*;
use gtk4::{
    gdk, glib, Align, ApplicationWindow, Box as GtkBox, Button, EventControllerKey, Image, Label,
    MenuButton, Orientation, Overlay, PolicyType, ScrolledWindow, TextView, WrapMode,
};
use libadwaita as adw;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Height the input grows to before it scrolls
const INPUT_MAX_HEIGHT: i32 = 132;

/// Shared state for chat (passed explicitly between content and input builders)
#[derive(Clone)]
pub(crate) struct ChatSharedState {
    pub(crate) transcript: Transcript,
    empty: EmptyState,
    status: StatusBar,
    transcript_label: Label,
    attachment: Attachment,
    recorder: Rc<RefCell<AudioRecorder>>,
    listener: Rc<RefCell<ListenRecorder>>,
}

/// First screen of an empty chat: the logo and the name, or the way to settings
/// while there is no API key.
#[derive(Clone)]
struct EmptyState {
    root: GtkBox,
    setup: GtkBox,
}

impl EmptyState {
    fn new() -> Self {
        let root = GtkBox::new(Orientation::Vertical, 6);
        root.set_valign(Align::Center);
        root.set_margin_start(16);
        root.set_margin_end(16);

        let logo = gdk::Texture::from_bytes(&glib::Bytes::from_static(include_bytes!(
            "../../../../icons/shaya.png"
        )));
        if let Ok(logo) = logo {
            let image = Image::from_paintable(Some(&logo));
            image.set_pixel_size(56);
            image.set_margin_bottom(6);
            root.append(&image);
        }

        let title = Label::new(Some("Shaya"));
        title.add_css_class("empty-title");
        root.append(&title);

        let setup = GtkBox::new(Orientation::Vertical, 10);
        let description = Label::new(Some("Add an API key or a local server to start chatting."));
        description.add_css_class("empty-description");
        description.set_wrap(true);
        description.set_justify(gtk4::Justification::Center);
        setup.append(&description);

        let open = Button::with_label("Open Settings");
        open.add_css_class("pill");
        open.add_css_class("suggested-action");
        open.set_halign(Align::Center);
        open.set_detailed_action_name("nav.section::settings");
        setup.append(&open);
        root.append(&setup);

        Self { root, setup }
    }

    fn refresh(&self, settings: &Settings) {
        self.setup
            .set_visible(settings.chat_route(false).missing().is_some());
    }
}

/// Build the transcript area.
/// Returns the widget and shared state needed by `build_input`.
pub fn build_content(state: Rc<RefCell<AppState>>) -> (GtkBox, ChatSharedState) {
    let vbox = GtkBox::new(Orientation::Vertical, 0);
    vbox.set_vexpand(true);

    let status = StatusBar::new();
    if let Some(error) = &state.borrow().load_error {
        status.error(error);
    }

    let transcript_label = Label::new(None);
    transcript_label.set_halign(Align::Start);
    transcript_label.set_selectable(true);
    transcript_label.set_wrap(true);
    transcript_label.set_max_width_chars(40);
    transcript_label.add_css_class("dim-label");
    transcript_label.set_visible(false);

    let empty = EmptyState::new();
    let transcript = Transcript::new(&empty.root, markdown::create_tag_table());
    vbox.append(transcript.widget());

    empty.refresh(&state.borrow().settings);
    transcript.update(&state.borrow());

    let shared = ChatSharedState {
        transcript,
        empty,
        status,
        transcript_label,
        attachment: Attachment::new(),
        recorder: Rc::new(RefCell::new(AudioRecorder::new())),
        listener: Rc::new(RefCell::new(ListenRecorder::new())),
    };

    (vbox, shared)
}

fn tool_button(icon: &str, tooltip: &str) -> Button {
    let button = Button::from_icon_name(icon);
    button.add_css_class("flat");
    button.add_css_class("composer-tool");
    button.set_tooltip_text(Some(tooltip));
    button
}

/// Multi-line input with a placeholder. Returns the view and its overlay.
fn build_text_input() -> (TextView, Overlay) {
    let input = TextView::builder()
        .wrap_mode(WrapMode::WordChar)
        .accepts_tab(false)
        .top_margin(4)
        .bottom_margin(4)
        .left_margin(6)
        .right_margin(6)
        .build();
    input.add_css_class("composer-input");

    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .propagate_natural_height(true)
        .max_content_height(INPUT_MAX_HEIGHT)
        .child(&input)
        .build();

    let placeholder = Label::new(Some("Message Shaya…"));
    placeholder.add_css_class("composer-placeholder");
    placeholder.set_halign(Align::Start);
    placeholder.set_valign(Align::Start);
    placeholder.set_margin_top(4);
    placeholder.set_margin_start(6);
    placeholder.set_can_target(false);

    input.buffer().connect_changed({
        let placeholder = placeholder.clone();
        move |buffer| placeholder.set_visible(buffer.char_count() == 0)
    });

    let overlay = Overlay::new();
    overlay.set_child(Some(&scroll));
    overlay.add_overlay(&placeholder);
    (input, overlay)
}

/// Enter sends and Shift+Enter starts a new line. An input method keeps Enter
/// while it is composing text, since there Enter confirms the composition.
fn connect_enter_to_send(input: &TextView, send: impl Fn() + 'static) {
    let composing = Rc::new(Cell::new(false));
    input.connect_preedit_changed({
        let composing = composing.clone();
        move |_, preedit| composing.set(!preedit.is_empty())
    });

    let keys = EventControllerKey::new();
    keys.set_propagation_phase(gtk4::PropagationPhase::Capture);
    keys.connect_key_pressed(move |_, key, _, modifiers| {
        let is_enter = matches!(
            key,
            gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::ISO_Enter
        );
        if is_enter && !modifiers.contains(gdk::ModifierType::SHIFT_MASK) && !composing.get() {
            send();
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    input.add_controller(keys);
}

/// Build interactive input area (control panel - receives input)
pub fn build_input(
    window: &ApplicationWindow,
    state: Rc<RefCell<AppState>>,
    stack: &adw::ViewStack,
    shared: ChatSharedState,
) -> GtkBox {
    let ChatSharedState {
        transcript,
        empty,
        status,
        transcript_label,
        attachment,
        recorder,
        listener,
    } = shared;

    let (input, input_area) = build_text_input();
    let buffer = input.buffer();

    let send_btn = Button::from_icon_name("go-up-symbolic");
    send_btn.add_css_class("suggested-action");
    send_btn.add_css_class("circular");
    send_btn.add_css_class("composer-send");
    send_btn.set_valign(Align::End);
    send_btn.set_tooltip_text(Some("Send (Enter)"));

    let chat_ctx = messaging::ChatContext {
        input: buffer.clone(),
        state: state.clone(),
        transcript: transcript.clone(),
        status: status.clone(),
        send_btn: send_btn.clone(),
        attachment: attachment.clone(),
    };

    {
        let ctx = chat_ctx.clone();
        send_btn.connect_clicked(move |_| messaging::send_message(&ctx));
    }
    connect_enter_to_send(&input, move || messaging::send_message(&chat_ctx));

    let audio_btn = tool_button("audio-input-microphone-symbolic", "Record and transcribe");
    {
        let recorder_c = recorder.clone();
        let state_c = state.clone();
        let status_c = status.clone();
        let buffer_c = buffer.clone();
        audio_btn.connect_clicked(move |btn| {
            audio::handle_audio_click(&recorder_c, &state_c, &status_c, btn, &buffer_c);
        });
    }

    let listen_btn = tool_button("audio-volume-high-symbolic", "Capture system audio");
    {
        let listener_c = listener.clone();
        let state_c = state.clone();
        let status_c = status.clone();
        let buffer_c = buffer.clone();
        listen_btn.connect_clicked(move |btn| {
            audio::handle_listen_click(&listener_c, &state_c, &status_c, btn, &buffer_c);
        });
    }

    let live_btn = Button::with_label("Live");
    live_btn.add_css_class("flat");
    live_btn.add_css_class("composer-tool");
    live_btn.set_tooltip_text(Some("Start Live Assistant"));

    let live_runtime = Rc::new(RefCell::new(live::LiveRuntime::default()));
    let live_ctx = live::LiveContext {
        state: state.clone(),
        transcript: transcript.clone(),
        status: status.clone(),
        transcript_label: transcript_label.clone(),
        attachment: attachment.clone(),
        audio_btn: audio_btn.clone(),
        listen_btn: listen_btn.clone(),
        live_btn: live_btn.clone(),
        recorder,
        listener,
    };

    {
        let ctx = live_ctx.clone();
        let runtime = live_runtime.clone();
        live_btn.connect_clicked(move |_| live::toggle(ctx.clone(), runtime.clone()));
    }
    status.answer_btn.connect_clicked(move |_| {
        live::request_manual_answer(live_ctx.clone(), live_runtime.clone());
    });

    let capture_menu = gio::Menu::new();
    capture_menu.append(Some("Select Area"), Some("chat.capture-area"));
    capture_menu.append(Some("Full Screen"), Some("chat.capture-screen"));
    let capture_btn = MenuButton::builder()
        .icon_name("camera-photo-symbolic")
        .tooltip_text("Capture screen")
        .menu_model(&capture_menu)
        .has_frame(false)
        .css_classes(["composer-tool"])
        .build();

    let model_label = Label::new(None);
    model_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    let model_chip = Button::builder()
        .child(&model_label)
        .tooltip_text("Model. Change it in Settings")
        .action_name("nav.section")
        .action_target(&"settings".to_variant())
        .css_classes(["flat", "model-chip"])
        .build();
    model_chip.set_hexpand(true);
    model_chip.set_halign(Align::End);
    model_chip.set_valign(Align::Center);

    let tools = GtkBox::new(Orientation::Horizontal, 2);
    tools.append(&capture_btn);
    tools.append(&audio_btn);
    tools.append(&listen_btn);
    tools.append(&live_btn);
    tools.append(&model_chip);
    tools.append(&send_btn);

    let composer = GtkBox::new(Orientation::Vertical, 4);
    composer.add_css_class("composer");
    composer.append(attachment.widget());
    composer.append(&input_area);
    composer.append(&tools);

    let vbox = GtkBox::new(Orientation::Vertical, 6);
    vbox.append(&transcript_label);
    vbox.append(&status.root);
    vbox.append(&composer);

    // Actions for the menus and the empty state
    let actions = gio::SimpleActionGroup::new();
    let add_action = |name: &str, activate: Box<dyn Fn()>| {
        let action = gio::SimpleAction::new(name, None);
        action.connect_activate(move |_, _| activate());
        actions.add_action(&action);
        action
    };
    // An action reaches its handler while the button is insensitive, so it follows the button.
    let follow = |btn: &gtk4::Widget, action: &gio::SimpleAction| {
        btn.bind_property("sensitive", action, "enabled")
            .sync_create()
            .build();
    };
    let capture_area = add_action("capture-area", {
        let (btn, attachment, status) = (capture_btn.clone(), attachment.clone(), status.clone());
        Box::new(move || messaging::handle_area_capture_click(&btn, &attachment, &status))
    });
    let capture_screen = add_action("capture-screen", {
        let (btn, attachment, status) = (capture_btn.clone(), attachment.clone(), status.clone());
        Box::new(move || messaging::handle_screenshot_click(&btn, &attachment, &status))
    });
    follow(capture_btn.upcast_ref(), &capture_area);
    follow(capture_btn.upcast_ref(), &capture_screen);
    let dictate = add_action("dictate", {
        let btn = audio_btn.clone();
        Box::new(move || btn.emit_clicked())
    });
    follow(audio_btn.upcast_ref(), &dictate);
    add_action("live", {
        let btn = live_btn.clone();
        Box::new(move || btn.emit_clicked())
    });
    add_action("save", {
        let (state, status) = (state.clone(), status.clone());
        Box::new(move || save_to_history(&state, &status))
    });
    add_action("clear", {
        let (state, transcript, attachment) =
            (state.clone(), transcript.clone(), attachment.clone());
        let buffer = buffer.clone();
        Box::new(move || {
            state.borrow_mut().replace_chat(Vec::new());
            attachment.clear();
            buffer.set_text("");
            transcript.update(&state.borrow());
        })
    });
    window.insert_action_group("chat", Some(&actions));

    // Settings can change the model and the API key while the chat is away.
    let refresh = {
        let state = state.clone();
        move || {
            let state = state.borrow();
            let model = display_name(state.settings.active_model());
            model_label.set_text(model);
            empty.refresh(&state.settings);
        }
    };
    refresh();
    stack.connect_visible_child_name_notify(move |stack| {
        if stack.visible_child_name().as_deref() == Some("chat") {
            refresh();
            input.grab_focus();
        }
    });

    vbox
}

fn save_to_history(state: &Rc<RefCell<AppState>>, status: &StatusBar) {
    let mut state = state.borrow_mut();
    if state.current_messages.is_empty() {
        status.flash("Nothing to save");
        return;
    }

    // Generate title from first user message (max 50 chars)
    let title = state
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

    let now = Utc::now();
    let conversation = Conversation {
        id: now.timestamp_millis().to_string(),
        title,
        messages: state.current_messages.clone(),
        created_at: now,
    };

    state.history.push(conversation);
    if let Err(e) = state.save() {
        tracing::error!("Failed to save history: {}", e);
        status.error("Save failed");
    } else {
        status.flash("Saved to history");
    }
}
