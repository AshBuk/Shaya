// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Status above the composer: a transient message and the active audio capture.

use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, Orientation, Spinner};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

type RetryAction = Box<dyn FnOnce()>;

#[derive(Clone)]
pub(crate) struct StatusBar {
    pub root: GtkBox,
    message: GtkBox,
    spinner: Spinner,
    label: Label,
    retry: Button,
    /// Runs on Retry. Dropped with the error it belongs to.
    retry_action: Rc<RefCell<Option<RetryAction>>>,
    dismiss: Button,
    activity: GtkBox,
    activity_label: Label,
    pub answer_btn: Button,
    /// Active captures with their text, latest last. Microphone and system audio
    /// can run together. Stop acts through the toggle of the latest one.
    sources: Rc<RefCell<Vec<(Button, String)>>>,
    timer: Rc<Cell<Option<glib::SourceId>>>,
}

impl StatusBar {
    pub fn new() -> Self {
        let root = GtkBox::new(Orientation::Vertical, 4);

        let spinner = Spinner::new();
        spinner.set_visible(false);

        let label = Label::new(None);
        label.set_hexpand(true);
        label.set_xalign(0.0);
        label.set_wrap(true);
        label.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
        label.set_selectable(true);
        label.set_focusable(false);

        let retry = Button::with_label("Retry");
        retry.add_css_class("flat");
        retry.add_css_class("small-action");
        retry.set_valign(Align::Start);
        retry.set_visible(false);

        let dismiss = Button::from_icon_name("window-close-symbolic");
        dismiss.add_css_class("flat");
        dismiss.add_css_class("small-action");
        dismiss.set_valign(Align::Start);
        dismiss.set_tooltip_text(Some("Dismiss"));

        let message = GtkBox::new(Orientation::Horizontal, 6);
        message.add_css_class("chat-status");
        message.set_visible(false);
        message.append(&spinner);
        message.append(&label);
        message.append(&retry);
        message.append(&dismiss);

        let dot = GtkBox::new(Orientation::Horizontal, 0);
        dot.add_css_class("activity-dot");
        dot.set_valign(Align::Center);

        let activity_label = Label::new(None);
        activity_label.set_hexpand(true);
        activity_label.set_xalign(0.0);
        activity_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);

        let answer_btn = Button::with_label("Answer now");
        answer_btn.add_css_class("flat");
        answer_btn.set_tooltip_text(Some("Answer the latest live transcript now"));

        let stop_btn = Button::with_label("Stop");
        stop_btn.add_css_class("flat");

        let activity = GtkBox::new(Orientation::Horizontal, 8);
        activity.add_css_class("activity-bar");
        activity.set_visible(false);
        activity.append(&dot);
        activity.append(&activity_label);
        activity.append(&answer_btn);
        activity.append(&stop_btn);

        root.append(&activity);
        root.append(&message);

        let status = Self {
            root,
            message,
            spinner,
            label,
            retry,
            retry_action: Rc::default(),
            dismiss,
            activity,
            activity_label,
            answer_btn,
            sources: Rc::default(),
            timer: Rc::default(),
        };

        let sources = status.sources.clone();
        stop_btn.connect_clicked(move |_| {
            let toggle = sources.borrow().last().map(|(toggle, _)| toggle.clone());
            if let Some(toggle) = toggle {
                toggle.emit_clicked();
            }
        });

        let status_c = status.clone();
        status.dismiss.connect_clicked(move |_| status_c.clear());

        let action = status.retry_action.clone();
        status.retry.connect_clicked(move |_| {
            let run = action.borrow_mut().take();
            if let Some(run) = run {
                run();
            }
        });

        status
    }

    /// Work in progress. Stays until replaced or cleared.
    pub fn busy(&self, text: &str) {
        self.show(text, "neutral");
        self.spinner.set_visible(true);
        self.spinner.start();
    }

    /// A failure. Stays until dismissed or replaced.
    pub fn error(&self, text: &str) {
        self.show(text, "error");
        self.dismiss.set_visible(true);
    }

    /// A failure the user can retry with `action`.
    pub fn error_with_retry(&self, text: &str, action: impl FnOnce() + 'static) {
        self.error(text);
        *self.retry_action.borrow_mut() = Some(Box::new(action));
        self.retry.set_visible(true);
    }

    /// A short note that hides by itself.
    pub fn flash(&self, text: &str) {
        self.show(text, "neutral");
        let status = self.clone();
        let id = glib::timeout_add_local_once(Duration::from_secs(2), move || {
            status.timer.set(None);
            status.clear();
        });
        self.timer.set(Some(id));
    }

    pub fn clear(&self) {
        self.cancel_timer();
        self.spinner.stop();
        self.drop_retry();
        self.message.set_visible(false);
    }

    /// Show the active capture with its stop button.
    pub fn start_activity(&self, text: &str, source: &Button, can_answer: bool) {
        self.sources
            .borrow_mut()
            .push((source.clone(), text.to_string()));
        self.activity_label.set_text(text);
        self.answer_btn.set_visible(can_answer);
        self.activity.set_visible(true);
    }

    /// Update the capture text. A previous failure is over once capture moves on.
    pub fn activity(&self, text: &str) {
        self.activity_label.set_text(text);
        if self.message.has_css_class("error") {
            self.clear();
        }
    }

    /// Remove a stopped capture and show the one still running, if any.
    /// Only Live can answer, and it never runs next to another capture.
    pub fn stop_activity(&self, source: &Button) {
        let mut sources = self.sources.borrow_mut();
        sources.retain(|(toggle, _)| toggle != source);
        if let Some((_, text)) = sources.last() {
            self.activity_label.set_text(text);
        }
        self.answer_btn.set_visible(false);
        self.activity.set_visible(!sources.is_empty());
    }

    fn show(&self, text: &str, class: &str) {
        self.cancel_timer();
        self.spinner.stop();
        self.spinner.set_visible(false);
        self.dismiss.set_visible(false);
        self.drop_retry();
        self.label.set_text(text);
        self.message.set_css_classes(&["chat-status", class]);
        self.message.set_visible(true);
    }

    fn drop_retry(&self) {
        self.retry.set_visible(false);
        // Take first: dropping the action can touch the status bar.
        let action = self.retry_action.borrow_mut().take();
        drop(action);
    }

    fn cancel_timer(&self) {
        if let Some(id) = self.timer.take() {
            id.remove();
        }
    }
}
