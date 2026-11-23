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
use gtk4::gio;
use gtk4::glib;
use gtk4::{
    Box as GtkBox, Button, Label, ListBox, Orientation, PolicyType, ScrolledWindow, SelectionMode,
};
use libadwaita as adw;
use libadwaita::prelude::MessageDialogExt;
use std::cell::RefCell;
use std::rc::Rc;

/// Build the meetings view
pub fn build(state: Rc<RefCell<AppState>>, stack: &adw::ViewStack) -> GtkBox {
    let vbox = GtkBox::new(Orientation::Vertical, 0);
    vbox.set_vexpand(true);

    MeetingManager::prepare_storage();

    // Shared references for clean refresh
    let recorder: Rc<RefCell<Option<MeetingRecorder>>> = Rc::new(RefCell::new(None));
    let list_container = Rc::new(GtkBox::new(Orientation::Vertical, 0));
    list_container.set_vexpand(true);

    // Recording controls (pass list_container for reliable refresh)
    let controls = build_controls(&state, &recorder, &list_container);
    vbox.append(&controls);
    vbox.append(list_container.as_ref());

    rebuild_list(&list_container, &state);

    // Auto-refresh when view becomes visible
    let list_c = list_container.clone();
    let state_c = state.clone();
    stack.connect_visible_child_name_notify(move |s| {
        if s.visible_child_name().as_deref() == Some("meetings") {
            rebuild_list(&list_c, &state_c);
        }
    });

    vbox
}

/// Build recording controls (start/stop + mic toggle + status)
fn build_controls(
    state: &Rc<RefCell<AppState>>,
    recorder: &Rc<RefCell<Option<MeetingRecorder>>>,
    list_container: &Rc<GtkBox>,
) -> GtkBox {
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
    mic_toggle.set_active(state.borrow().settings.meeting_record_mic);
    mic_toggle.set_width_request(140);

    // Persist mic toggle state to disk
    let state_c = state.clone();
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
    let state_c = state.clone();
    let status_c = status_label.clone();
    let mic_c = mic_toggle.clone();
    let timer_id_c = timer_id.clone();
    let list_c = list_container.clone();

    record_btn.connect_clicked(move |btn| {
        let is_recording = recorder_c.borrow().is_some();

        if is_recording {
            // Stop recording
            let meeting = {
                let mut rec = recorder_c.borrow_mut();
                rec.as_mut().and_then(|r| r.stop().ok())
            };
            *recorder_c.borrow_mut() = None;

            // Stop timer
            if let Some(id) = timer_id_c.borrow_mut().take() {
                id.remove();
            }

            btn.set_label("Start Recording");
            btn.remove_css_class("destructive-action");
            btn.add_css_class("suggested-action");
            mic_c.set_sensitive(true);

            if let Some(m) = meeting {
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
                if let Ok(mut meta) = MeetingManager::load(&m.id) {
                    meta.status = MeetingStatus::Converting;
                    let _ = MeetingManager::save(&meta);
                }

                let meeting_id = m.id.clone();
                let list_cc = list_c.clone();
                let state_cc = state_c.clone();
                glib::spawn_future_local(async move {
                    let id = meeting_id.clone();
                    let _ = gio::spawn_blocking(move || {
                        if convert_meeting_to_opus(&id) {
                            if let Ok(mut m) = MeetingManager::load(&id) {
                                m.audio_format = "opus".to_string();
                                m.status = MeetingStatus::Completed;
                                let _ = MeetingManager::save(&m);
                            }
                        } else {
                            // No converter available — mark as Completed anyway
                            if let Ok(mut m) = MeetingManager::load(&id) {
                                m.status = MeetingStatus::Completed;
                                let _ = MeetingManager::save(&m);
                            }
                        }
                    })
                    .await;
                    rebuild_list(&list_cc, &state_cc);
                });
            } else {
                status_c.set_text("✅ Recording stopped");
            }

            rebuild_list(&list_c, &state_c);
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
                    status_c.set_text(&format!("❌ {}", e));
                    status_c.set_visible(true);
                }
            }
        }
    });
    controls
}

/// Rebuild the meeting list
fn rebuild_list(container: &GtkBox, state: &Rc<RefCell<AppState>>) {
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
            let row = build_meeting_row(meeting, state, container);
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

/// Build a single meeting row: title/subtitle + action buttons
fn build_meeting_row(
    meeting: &crate::core::meeting::Meeting,
    state: &Rc<RefCell<AppState>>,
    container: &GtkBox,
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

    row.add_suffix(&build_meeting_actions(meeting, state, container));
    row
}

/// Build the full action button strip for a meeting row:
/// play | [transcription controls] | delete
fn build_meeting_actions(
    meeting: &crate::core::meeting::Meeting,
    state: &Rc<RefCell<AppState>>,
    container: &GtkBox,
) -> GtkBox {
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
                    tracing::error!("Failed to open audio: {}", e);
                }
            }
        });
        actions.append(&play_btn);
    }

    // Status-dependent transcription pipeline buttons
    actions.append(&build_transcription_controls(meeting, state, container));

    // Recording, conversion and transcription still write to the meeting directory.
    if matches!(
        meeting.status,
        MeetingStatus::Recording | MeetingStatus::Converting | MeetingStatus::Transcribing
    ) {
        return actions;
    }

    let delete_btn = Button::from_icon_name("user-trash-symbolic");
    delete_btn.set_tooltip_text(Some("Delete meeting"));
    delete_btn.add_css_class("flat");
    delete_btn.set_valign(gtk4::Align::Center);

    let meeting_id = meeting.id.clone();
    let state_c = state.clone();
    let container_c = container.clone();

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
        let state_cc = state_c.clone();
        let container_cc = container_c.clone();

        dialog.connect_response(None, move |d, response| {
            if response == "delete" {
                if let Err(e) = MeetingManager::delete(&meeting_id_c) {
                    tracing::error!("Failed to delete meeting: {}", e);
                }
                rebuild_list(&container_cc, &state_cc);
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
    state: &Rc<RefCell<AppState>>,
    container: &GtkBox,
) -> GtkBox {
    let controls = GtkBox::new(Orientation::Horizontal, 4);
    controls.set_valign(gtk4::Align::Center);

    // Transcribe button (only if completed, not yet transcribed)
    if meeting.status == MeetingStatus::Completed {
        let transcribe_btn = Button::from_icon_name("document-edit-symbolic");
        transcribe_btn.set_tooltip_text(Some("Transcribe"));
        transcribe_btn.add_css_class("flat");
        transcribe_btn.set_valign(gtk4::Align::Center);

        let meeting_id = meeting.id.clone();
        let state_c = state.clone();
        let container_c = container.clone();

        transcribe_btn.connect_clicked(move |btn| {
            btn.set_sensitive(false);
            btn.set_tooltip_text(Some("Transcribing..."));

            let (stt_provider, stt_key, stt_model) = {
                let s = state_c.borrow();
                (
                    s.settings.stt_provider,
                    s.settings.active_stt_key().to_string(),
                    s.settings.active_stt_model().to_string(),
                )
            };

            if stt_key.is_empty() {
                btn.set_sensitive(true);
                btn.set_tooltip_text(Some("Set STT API key in Settings first"));
                return;
            }

            if let Ok(mut m) = MeetingManager::load(&meeting_id) {
                m.status = MeetingStatus::Transcribing;
                m.last_error = None;
                let _ = MeetingManager::save(&m);
            }

            let meeting_id_c = meeting_id.clone();
            let state_cc = state_c.clone();
            let container_cc = container_c.clone();

            glib::spawn_future_local(async move {
                let id = meeting_id_c.clone();
                let result = gio::spawn_blocking(move || {
                    meeting::transcribe_meeting(&id, stt_provider, &stt_key, &stt_model)
                        .map_err(|e| e.to_string())
                })
                .await;

                let failure = match result {
                    Ok(Ok(_)) => None,
                    Ok(Err(e)) => Some(e),
                    Err(e) => {
                        tracing::error!("Transcription task failed: {:?}", e);
                        Some("Transcription task failed".to_string())
                    }
                };
                if let Some(failure) = failure {
                    tracing::error!("Transcription failed for {}: {}", meeting_id_c, failure);
                    if let Ok(mut m) = MeetingManager::load(&meeting_id_c) {
                        m.status = MeetingStatus::Completed;
                        m.last_error = Some(failure);
                        let _ = MeetingManager::save(&m);
                    }
                }
                rebuild_list(&container_cc, &state_cc);
            });
            rebuild_list(&container_c, &state_c);
        });
        controls.append(&transcribe_btn);
    }

    // Analyze button (only if transcribed, not yet analyzed)
    if meeting.status == MeetingStatus::Transcribed {
        let analyze_btn = Button::from_icon_name("starred-symbolic");
        analyze_btn.set_tooltip_text(Some("Analyze with AI"));
        analyze_btn.add_css_class("flat");
        analyze_btn.set_valign(gtk4::Align::Center);

        let meeting_id = meeting.id.clone();
        let state_c = state.clone();
        let container_c = container.clone();

        analyze_btn.connect_clicked(move |btn| {
            btn.set_sensitive(false);

            let route = state_c.borrow().settings.chat_route(false);
            if let Some(missing) = route.missing() {
                btn.set_sensitive(true);
                btn.set_tooltip_text(Some(missing));
                return;
            }

            let meeting_id_c = meeting_id.clone();
            let state_cc = state_c.clone();
            let container_cc = container_c.clone();

            glib::spawn_future_local(async move {
                let id = meeting_id_c.clone();
                let result = gio::spawn_blocking(move || {
                    meeting::analyze_meeting(&id, &route).map_err(|e| e.to_string())
                })
                .await;

                let failure = match result {
                    Ok(Ok(_)) => None,
                    Ok(Err(e)) => Some(e),
                    Err(e) => {
                        tracing::error!("Analysis task failed: {:?}", e);
                        Some("Analysis task failed".to_string())
                    }
                };
                if let Ok(mut m) = MeetingManager::load(&meeting_id_c) {
                    if let Some(failure) = &failure {
                        tracing::error!("Analysis failed: {}", failure);
                    }
                    m.last_error = failure;
                    let _ = MeetingManager::save(&m);
                }

                rebuild_list(&container_cc, &state_cc);
            });
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

/// Open audio file with the system's default player
fn open_with_system(path: &std::path::Path) -> Result<(), String> {
    std::process::Command::new("xdg-open")
        .arg(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("xdg-open failed: {}", e))?;
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

    let is_dark = gtk4::gdk::Display::default()
        .map(|d| libadwaita::StyleManager::for_display(&d).is_dark())
        .unwrap_or(true);
    let tag_table = markdown::create_tag_table(is_dark);
    let buffer = gtk4::TextBuffer::new(Some(&tag_table));
    markdown::render(&buffer, content);

    let text_view = gtk4::TextView::with_buffer(&buffer);
    text_view.set_editable(false);
    text_view.set_wrap_mode(gtk4::WrapMode::Word);
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
