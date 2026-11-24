// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Meetings View - Record, transcribe, and analyze meetings

use crate::core::audio::meet_recorder::{convert_meeting_to_opus, MeetingRecorder};
use crate::core::meeting::{self, MeetingManager, MeetingStatus};
use crate::core::state::AppState;
use adw::prelude::*;
use anyhow::Context;
use gtk4::gio;
use gtk4::glib;
use gtk4::{
    Box as GtkBox, Button, Label, ListBox, Orientation, PolicyType, ScrolledWindow, SelectionMode,
};
use libadwaita as adw;
use libadwaita::prelude::MessageDialogExt;
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::rc::Rc;

/// A paid request running for a meeting.
#[derive(Clone, Copy)]
enum Operation {
    Transcription,
    Analysis,
}

impl Operation {
    fn label(self) -> &'static str {
        match self {
            Self::Transcription => "Transcription",
            Self::Analysis => "Analysis",
        }
    }
}

/// Operations in progress by meeting ID. Rows are rebuilt from files on every
/// refresh, so this outlives them and decides what their buttons offer.
#[derive(Clone, Default)]
struct Operations(Rc<RefCell<HashMap<String, Operation>>>);

impl Operations {
    fn running(&self, meeting_id: &str) -> Option<Operation> {
        self.0.borrow().get(meeting_id).copied()
    }

    /// Claim the meeting for `operation`, or `None` while another one runs on it.
    fn start(&self, meeting_id: &str, operation: Operation) -> Option<Running> {
        match self.0.borrow_mut().entry(meeting_id.to_string()) {
            Entry::Occupied(_) => return None,
            Entry::Vacant(slot) => slot.insert(operation),
        };
        Some(Running {
            operations: self.clone(),
            meeting_id: meeting_id.to_string(),
            operation,
        })
    }
}

/// A claimed meeting, released on drop.
struct Running {
    operations: Operations,
    meeting_id: String,
    operation: Operation,
}

impl Drop for Running {
    fn drop(&mut self) {
        self.operations.0.borrow_mut().remove(&self.meeting_id);
    }
}

/// The meeting list with what its rows need to act.
#[derive(Clone)]
struct MeetingList {
    container: GtkBox,
    state: Rc<RefCell<AppState>>,
    operations: Operations,
}

/// Build the meetings view
pub fn build(state: Rc<RefCell<AppState>>, stack: &adw::ViewStack) -> GtkBox {
    let vbox = GtkBox::new(Orientation::Vertical, 0);
    vbox.set_vexpand(true);

    MeetingManager::prepare_storage();

    let recorder: Rc<RefCell<Option<MeetingRecorder>>> = Rc::new(RefCell::new(None));
    let list = MeetingList {
        container: GtkBox::new(Orientation::Vertical, 0),
        state,
        operations: Operations::default(),
    };
    list.container.set_vexpand(true);

    let controls = build_controls(&recorder, &list);
    vbox.append(&controls);
    vbox.append(&list.container);

    list.rebuild();

    // Auto-refresh when view becomes visible
    let list_c = list.clone();
    stack.connect_visible_child_name_notify(move |s| {
        if s.visible_child_name().as_deref() == Some("meetings") {
            list_c.rebuild();
        }
    });

    vbox
}

/// Build recording controls (start/stop + mic toggle + status)
fn build_controls(recorder: &Rc<RefCell<Option<MeetingRecorder>>>, list: &MeetingList) -> GtkBox {
    let controls = GtkBox::new(Orientation::Vertical, 4);
    controls.set_margin_top(8);
    controls.set_margin_start(12);
    controls.set_margin_end(12);
    controls.set_margin_bottom(4);

    // Row: record button + mic toggle
    let row = GtkBox::new(Orientation::Horizontal, 8);

    let record_btn = Button::with_label("Start Recording");
    record_btn.add_css_class("suggested-action");
    record_btn.set_hexpand(true);

    let mic_toggle = adw::SwitchRow::new();
    mic_toggle.set_title("Microphone");
    mic_toggle.set_active(list.state.borrow().settings.meeting_record_mic);
    mic_toggle.set_width_request(140);

    // Persist mic toggle state to disk
    let state_c = list.state.clone();
    mic_toggle.connect_active_notify(move |row| {
        state_c.borrow_mut().settings.meeting_record_mic = row.is_active();
        if let Err(e) = state_c.borrow().save() {
            tracing::error!("Failed to save state: {}", e);
        }
    });

    // AdwSwitchRow can only be placed into a GtkListBox
    let mic_list = ListBox::new();
    mic_list.set_selection_mode(SelectionMode::None);
    mic_list.add_css_class("boxed-list");
    mic_list.append(&mic_toggle);

    row.append(&record_btn);
    row.append(&mic_list);
    controls.append(&row);

    // Status label (timer / status messages)
    let status_label = Label::new(None);
    status_label.add_css_class("dim-label");
    status_label.set_visible(false);
    controls.append(&status_label);

    // Timer source ID for cleanup
    let timer_id: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));

    // Record button handler
    let recorder_c = recorder.clone();
    let status_c = status_label.clone();
    let mic_c = mic_toggle.clone();
    let timer_id_c = timer_id.clone();
    let list_c = list.clone();

    record_btn.connect_clicked(move |btn| {
        let is_recording = recorder_c.borrow().is_some();

        if is_recording {
            // Stop recording
            let stopped = recorder_c.borrow_mut().take().map(|mut r| r.stop());

            // Stop timer
            if let Some(id) = timer_id_c.borrow_mut().take() {
                id.remove();
            }

            btn.set_label("Start Recording");
            btn.remove_css_class("destructive-action");
            btn.add_css_class("suggested-action");
            mic_c.set_sensitive(true);

            match stopped {
                Some(Ok(m)) => {
                    let saved = format!(
                        "{} ({})",
                        m.title,
                        MeetingManager::format_duration(m.duration_secs)
                    );
                    status_c.set_text(&match &m.last_error {
                        Some(problem) => format!("⚠️ Saved with problems: {saved}: {problem}"),
                        None => format!("✅ Saved: {saved}"),
                    });

                    // Create OPUS copies in background, the WAVs stay for transcription.
                    // Set Converting status to block Transcribe button during conversion
                    MeetingManager::update(&m.id, |meta| meta.status = MeetingStatus::Converting);

                    let meeting_id = m.id.clone();
                    let list_cc = list_c.clone();
                    glib::spawn_future_local(async move {
                        let id = meeting_id.clone();
                        let task = gio::spawn_blocking(move || {
                            let converted = convert_meeting_to_opus(&id);
                            // Without a converter the WAVs stay, and the meeting is complete anyway.
                            MeetingManager::update(&id, |m| {
                                if converted {
                                    m.audio_format = "opus".to_string();
                                }
                                m.status = MeetingStatus::Completed;
                            });
                        })
                        .await;
                        if task.is_err() {
                            tracing::error!("Conversion task failed for {meeting_id}");
                        }
                        list_cc.rebuild();
                    });
                }
                Some(Err(e)) => status_c.set_text(&format!("❌ Recording not saved: {e:#}")),
                None => status_c.set_text("✅ Recording stopped"),
            }

            list_c.rebuild();
        } else {
            // Start recording
            let record_mic = mic_c.is_active();
            match MeetingRecorder::start(record_mic) {
                Ok(rec) => {
                    *recorder_c.borrow_mut() = Some(rec);
                    btn.set_label("Stop Recording");
                    btn.remove_css_class("suggested-action");
                    btn.add_css_class("destructive-action");
                    mic_c.set_sensitive(false);
                    status_c.set_text("🔴 Recording: 0m 00s");
                    status_c.set_visible(true);

                    // Start timer
                    let recorder_cc = recorder_c.clone();
                    let status_cc = status_c.clone();
                    let id =
                        glib::timeout_add_local(std::time::Duration::from_secs(1), move || {
                            let (elapsed, failed) = recorder_cc
                                .borrow_mut()
                                .as_mut()
                                .map_or((0, false), |r| (r.elapsed_secs(), r.capture_failed()));
                            let duration = MeetingManager::format_duration(elapsed);
                            status_cc.set_text(&if failed {
                                format!("⚠️ System audio capture stopped: {duration}")
                            } else {
                                format!("🔴 Recording: {duration}")
                            });
                            glib::ControlFlow::Continue
                        });
                    *timer_id_c.borrow_mut() = Some(id);
                }
                Err(e) => {
                    status_c.set_text(&format!("❌ {e:#}"));
                    status_c.set_visible(true);
                }
            }
        }
    });
    controls
}

impl MeetingList {
    /// Rebuild the rows from the files and the running operations.
    fn rebuild(&self) {
        let container = &self.container;
        while let Some(child) = container.first_child() {
            container.remove(&child);
        }

        let meetings = MeetingManager::list();

        if meetings.is_empty() {
            let status = adw::StatusPage::new();
            status.set_icon_name(Some("media-record-symbolic"));
            status.set_title("No Meetings Yet");
            status.set_description(Some("Start a recording to capture meeting audio"));

            let scroll = ScrolledWindow::builder()
                .hscrollbar_policy(PolicyType::Never)
                .vscrollbar_policy(PolicyType::Automatic)
                .vexpand(true)
                .child(&status)
                .build();
            container.append(&scroll);
        } else {
            let list = gtk4::ListBox::new();
            list.set_selection_mode(gtk4::SelectionMode::None);
            list.add_css_class("boxed-list");
            list.set_margin_top(8);
            list.set_margin_bottom(12);
            list.set_margin_start(12);
            list.set_margin_end(12);

            for meeting in &meetings {
                let row = build_meeting_row(meeting, self);
                list.append(&row);
            }

            let scroll = ScrolledWindow::builder()
                .hscrollbar_policy(PolicyType::Never)
                .vscrollbar_policy(PolicyType::Automatic)
                .vexpand(true)
                .child(&list)
                .build();

            container.append(&scroll);
        }
    }

    /// Run `work` for the claimed meeting on a worker thread. `done` gets the
    /// failure, if any, before the claim ends and the list shows the result.
    fn run<T: Send + 'static>(
        &self,
        running: Running,
        work: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
        done: impl FnOnce(Option<String>) + 'static,
    ) {
        let list = self.clone();
        glib::spawn_future_local(async move {
            let operation = running.operation.label();
            let failure = match gio::spawn_blocking(work).await {
                Ok(Ok(_)) => None,
                Ok(Err(e)) => Some(format!("{e:#}")),
                Err(e) => {
                    tracing::error!("{operation} task panicked: {e:?}");
                    Some(format!("{operation} task failed"))
                }
            };
            if let Some(failure) = &failure {
                tracing::error!("{operation} failed for {}: {failure}", running.meeting_id);
            }
            done(failure);
            drop(running);
            list.rebuild();
        });
        self.rebuild();
    }
}

/// Build a single meeting row: title/subtitle + action buttons
fn build_meeting_row(
    meeting: &crate::core::meeting::Meeting,
    list: &MeetingList,
) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    row.set_title(&meeting.title);

    let status_icon = match meeting.status {
        MeetingStatus::Completed => "📁",
        MeetingStatus::Converting => "🔄",
        MeetingStatus::Transcribing => "⏳",
        MeetingStatus::Transcribed => "📝",
        MeetingStatus::Analyzed => "✅",
        MeetingStatus::Recording => "🔴",
    };

    let mic_text = if meeting.has_mic { " 🎤" } else { "" };
    let problem = meeting
        .last_error
        .as_deref()
        .map_or(String::new(), |e| format!("\n⚠️ {e}"));
    // Provider error text is not Pango markup.
    row.set_use_markup(false);
    row.set_subtitle(&format!(
        "{} {} • {}{} • {}{}",
        status_icon,
        meeting.status,
        MeetingManager::format_duration(meeting.duration_secs),
        mic_text,
        meeting.started_at_display(),
        problem
    ));

    row.add_suffix(&build_meeting_actions(meeting, list));
    row
}

/// Build the full action button strip for a meeting row:
/// play | [transcription controls] | delete
fn build_meeting_actions(meeting: &crate::core::meeting::Meeting, list: &MeetingList) -> GtkBox {
    let actions = GtkBox::new(Orientation::Horizontal, 4);
    actions.set_valign(gtk4::Align::Center);

    // Play button (available for all non-recording meetings)
    if meeting.status != MeetingStatus::Recording {
        let play_btn = Button::from_icon_name("media-playback-start-symbolic");
        play_btn.set_tooltip_text(Some("Play recording"));
        play_btn.add_css_class("flat");
        play_btn.set_valign(gtk4::Align::Center);

        let meeting_id = meeting.id.clone();
        play_btn.connect_clicked(move |_| {
            if let Some(path) = MeetingManager::find_playback_file(&meeting_id) {
                if let Err(e) = open_with_system(&path) {
                    tracing::error!("Failed to open audio: {e:#}");
                }
            }
        });
        actions.append(&play_btn);
    }

    // Status-dependent transcription pipeline buttons
    actions.append(&build_transcription_controls(meeting, list));

    // Recording, conversion and running operations still write to the meeting directory.
    if list.operations.running(&meeting.id).is_some()
        || matches!(
            meeting.status,
            MeetingStatus::Recording | MeetingStatus::Converting | MeetingStatus::Transcribing
        )
    {
        return actions;
    }

    let delete_btn = Button::from_icon_name("user-trash-symbolic");
    delete_btn.set_tooltip_text(Some("Delete meeting"));
    delete_btn.add_css_class("flat");
    delete_btn.set_valign(gtk4::Align::Center);

    let meeting_id = meeting.id.clone();
    let list_c = list.clone();

    delete_btn.connect_clicked(move |btn| {
        let dialog = adw::MessageDialog::new(
            btn.root()
                .and_then(|r| r.downcast::<gtk4::Window>().ok())
                .as_ref(),
            Some("Delete meeting recording?"),
            Some("This will delete the audio, transcript, and analysis. This cannot be undone."),
        );
        dialog.add_response("cancel", "Cancel");
        dialog.add_response("delete", "Delete");
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));

        let meeting_id_c = meeting_id.clone();
        let list_cc = list_c.clone();

        dialog.connect_response(None, move |d, response| {
            if response == "delete" {
                if let Err(e) = MeetingManager::delete(&meeting_id_c) {
                    tracing::error!("Failed to delete meeting: {}", e);
                }
                list_cc.rebuild();
            }
            d.close();
        });
        dialog.present();
    });
    actions.append(&delete_btn);
    actions
}

/// Build status-dependent transcription pipeline buttons:
/// Transcribe (Completed) → Analyze (Transcribed) → View (Transcribed | Analyzed)
fn build_transcription_controls(
    meeting: &crate::core::meeting::Meeting,
    list: &MeetingList,
) -> GtkBox {
    let controls = GtkBox::new(Orientation::Horizontal, 4);
    controls.set_valign(gtk4::Align::Center);
    let running = list.operations.running(&meeting.id);

    // Transcribe button (only if completed, not yet transcribed)
    if meeting.status == MeetingStatus::Completed {
        let transcribe_btn = operation_button("document-edit-symbolic", "Transcribe", running);

        let meeting_id = meeting.id.clone();
        let list_c = list.clone();

        transcribe_btn.connect_clicked(move |btn| {
            let (stt_provider, stt_key, stt_model) = {
                let s = list_c.state.borrow();
                (
                    s.settings.stt_provider,
                    s.settings.active_stt_key().to_string(),
                    s.settings.active_stt_model().to_string(),
                )
            };
            if stt_key.is_empty() {
                btn.set_tooltip_text(Some("Set STT API key in Settings first"));
                return;
            }
            let Some(running) = list_c
                .operations
                .start(&meeting_id, Operation::Transcription)
            else {
                return;
            };

            MeetingManager::update(&meeting_id, |m| {
                m.status = MeetingStatus::Transcribing;
                m.last_error = None;
            });

            let id = meeting_id.clone();
            let failed_id = meeting_id.clone();
            list_c.run(
                running,
                move || meeting::transcribe_meeting(&id, stt_provider, &stt_key, &stt_model),
                move |failure| {
                    if let Some(failure) = failure {
                        MeetingManager::update(&failed_id, |m| {
                            m.status = MeetingStatus::Completed;
                            m.last_error = Some(failure);
                        });
                    }
                },
            );
        });
        controls.append(&transcribe_btn);
    }

    // Analyze button (only if transcribed, not yet analyzed)
    if meeting.status == MeetingStatus::Transcribed {
        let analyze_btn = operation_button("starred-symbolic", "Analyze with AI", running);

        let meeting_id = meeting.id.clone();
        let list_c = list.clone();

        analyze_btn.connect_clicked(move |btn| {
            let route = list_c.state.borrow().settings.chat_route(false);
            if let Some(missing) = route.missing() {
                btn.set_tooltip_text(Some(missing));
                return;
            }
            let Some(running) = list_c.operations.start(&meeting_id, Operation::Analysis) else {
                return;
            };

            let id = meeting_id.clone();
            let done_id = meeting_id.clone();
            list_c.run(
                running,
                move || meeting::analyze_meeting(&id, &route),
                move |failure| MeetingManager::update(&done_id, |m| m.last_error = failure),
            );
        });
        controls.append(&analyze_btn);
    }

    // View transcript/analysis button (if available)
    if matches!(
        meeting.status,
        MeetingStatus::Transcribed | MeetingStatus::Analyzed
    ) {
        let view_btn = Button::from_icon_name("document-open-symbolic");
        view_btn.set_tooltip_text(Some("View transcript"));
        view_btn.add_css_class("flat");
        view_btn.set_valign(gtk4::Align::Center);

        let meeting_id = meeting.id.clone();
        let meeting_title = meeting.title.clone();
        let has_analysis = meeting.status == MeetingStatus::Analyzed;

        view_btn.connect_clicked(move |btn| {
            let window = btn.root().and_then(|r| r.downcast::<gtk4::Window>().ok());

            let content = if has_analysis {
                let analysis = MeetingManager::get_analysis(&meeting_id).unwrap_or_default();
                let transcript = MeetingManager::get_transcript(&meeting_id).unwrap_or_default();
                format!(
                    "=== AI Analysis ===\n\n{}\n\n=== Transcript ===\n\n{}",
                    analysis, transcript
                )
            } else {
                MeetingManager::get_transcript(&meeting_id).unwrap_or_default()
            };

            show_transcript_dialog(window.as_ref(), &meeting_title, &content);
        });
        controls.append(&view_btn);
    }
    controls
}

/// A row button that starts an operation. While one runs on the meeting, it shows
/// that and stays inactive.
fn operation_button(icon: &str, tooltip: &str, running: Option<Operation>) -> Button {
    let button = Button::from_icon_name(icon);
    button.add_css_class("flat");
    button.set_valign(gtk4::Align::Center);
    match running {
        Some(operation) => {
            button.set_sensitive(false);
            button.set_tooltip_text(Some(&format!("{} in progress", operation.label())));
        }
        None => button.set_tooltip_text(Some(tooltip)),
    }
    button
}

/// Open audio file with the system's default player
fn open_with_system(path: &std::path::Path) -> anyhow::Result<()> {
    std::process::Command::new("xdg-open")
        .arg(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("xdg-open failed")?;
    Ok(())
}

/// Show a dialog with transcript/analysis content (rendered as markdown)
fn show_transcript_dialog(parent: Option<&gtk4::Window>, title: &str, content: &str) {
    use super::chat::markdown;

    let dialog = adw::Dialog::new();
    dialog.set_title(title);
    dialog.set_content_width(500);
    dialog.set_content_height(400);

    let toolbar_view = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    toolbar_view.add_top_bar(&header);

    let tag_table = markdown::create_tag_table();
    let buffer = gtk4::TextBuffer::new(Some(&tag_table));

    let text_view = gtk4::TextView::with_buffer(&buffer);
    text_view.set_editable(false);
    text_view.set_cursor_visible(false);
    text_view.set_wrap_mode(gtk4::WrapMode::WordChar);
    markdown::MarkdownView::new(&text_view).render(content);
    text_view.set_margin_top(8);
    text_view.set_margin_bottom(8);
    text_view.set_margin_start(12);
    text_view.set_margin_end(12);

    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .vexpand(true)
        .child(&text_view)
        .build();

    toolbar_view.set_content(Some(&scroll));
    dialog.set_child(Some(&toolbar_view));

    if let Some(w) = parent {
        dialog.present(Some(w));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_meeting_runs_one_operation_at_a_time() {
        let operations = Operations::default();

        let running = operations.start("m1", Operation::Analysis);
        assert!(running.is_some());
        assert!(operations.start("m1", Operation::Analysis).is_none());
        assert!(operations.start("m1", Operation::Transcription).is_none());
        assert!(operations.start("m2", Operation::Analysis).is_some());
        assert!(matches!(
            operations.running("m1"),
            Some(Operation::Analysis)
        ));

        drop(running);
        assert!(operations.running("m1").is_none());
        assert!(operations.start("m1", Operation::Analysis).is_some());
    }
}
