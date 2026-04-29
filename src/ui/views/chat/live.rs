// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Chat-integrated Live Assistant controls.

use super::messaging;
use crate::core::api::{LlmClient, WhisperClient};
use crate::core::audio::{analyze_audio_file, AudioRecorder, ListenRecorder, VadConfig};
use crate::core::live::{
    is_no_suggestion, live_transcript_message, LiveRequestRange, LiveSession, LiveSessionManager,
    TranscriptSegment, LIVE_SYSTEM_PROMPT,
};
use crate::core::state::{AppState, ImageData, Message, Role};
use chrono::{DateTime, Utc};
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Button, Label, TextView};
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

#[derive(Clone)]
pub(super) struct LiveContext {
    pub state: Rc<RefCell<AppState>>,
    pub chat_view: TextView,
    pub status_label: Label,
    pub transcript_label: Label,
    pub attached_image: Rc<RefCell<Option<ImageData>>>,
    pub audio_btn: Button,
    pub listen_btn: Button,
    pub live_btn: Button,
    pub reply_btn: Button,
    pub recorder: Rc<RefCell<AudioRecorder>>,
    pub listener: Rc<RefCell<ListenRecorder>>,
}

#[derive(Default)]
pub(super) struct LiveRuntime {
    session: Option<LiveSession>,
    recorder: Option<ListenRecorder>,
    next_seq: u64,
}

enum ChunkResult {
    Silent,
    Speech(String),
}

impl LiveRuntime {
    fn is_active(&self) -> bool {
        self.session.is_some()
    }
}

pub(super) fn toggle(ctx: LiveContext, runtime: Rc<RefCell<LiveRuntime>>) {
    if runtime.borrow().is_active() {
        stop(ctx, runtime);
    } else {
        start(ctx, runtime);
    }
}

pub(super) fn request_manual_answer(ctx: LiveContext, runtime: Rc<RefCell<LiveRuntime>>) {
    request_answer(ctx, runtime, true);
}

fn start(ctx: LiveContext, runtime: Rc<RefCell<LiveRuntime>>) {
    if ctx.recorder.borrow().is_recording() || ctx.listener.borrow().is_listening() {
        show_status(&ctx.status_label, "Stop current audio capture before Live");
        return;
    }

    let settings = {
        let state = ctx.state.borrow();
        if state.settings.active_stt_key().is_empty() {
            show_status(&ctx.status_label, "Set STT API key in Settings");
            return;
        }
        if state.settings.active_api_key().is_empty() {
            show_status(&ctx.status_label, "Set AI API key in Settings");
            return;
        }
        (
            state.settings.live_chunk_secs.clamp(3, 10),
            state.settings.provider,
            state.settings.active_model().to_string(),
            state.settings.stt_provider,
            state.settings.active_stt_model().to_string(),
        )
    };

    let session = LiveSession::new(settings.0, settings.1, settings.2, settings.3, settings.4);
    if let Err(e) = LiveSessionManager::save_metadata(&session.metadata) {
        show_status(&ctx.status_label, &format!("Live save failed: {}", e));
        return;
    }
    if let Err(e) = fs::create_dir_all(LiveSessionManager::chunks_dir(&session.metadata.id)) {
        show_status(&ctx.status_label, &format!("Live setup failed: {}", e));
        return;
    }

    {
        let mut live = runtime.borrow_mut();
        live.session = Some(session);
        live.next_seq = 1;
    }

    set_live_buttons(&ctx, true);
    show_status(&ctx.status_label, "Live: listening");
    start_next_chunk(ctx, runtime);
}

fn stop(ctx: LiveContext, runtime: Rc<RefCell<LiveRuntime>>) {
    let mut session_to_save = None;
    {
        let mut live = runtime.borrow_mut();
        if let Some(mut recorder) = live.recorder.take() {
            let _ = recorder.stop();
        }
        if let Some(mut session) = live.session.take() {
            session.metadata.ended_at = Some(Utc::now());
            session_to_save = Some(session.metadata);
        }
    }

    if let Some(metadata) = session_to_save {
        let _ = LiveSessionManager::save_metadata(&metadata);
        let _ = LiveSessionManager::cleanup_chunks(&metadata.id);
    }

    set_live_buttons(&ctx, false);
    ctx.transcript_label.set_visible(false);
    show_status(&ctx.status_label, "Live: stopped");
}

fn start_next_chunk(ctx: LiveContext, runtime: Rc<RefCell<LiveRuntime>>) {
    let (seq, session_id, chunk_secs) = {
        let mut live = runtime.borrow_mut();
        let Some(session) = live.session.as_ref() else {
            return;
        };
        let session_id = session.metadata.id.clone();
        let chunk_secs = session.metadata.chunk_secs;
        let seq = live.next_seq;
        live.next_seq += 1;
        (seq, session_id, chunk_secs)
    };

    let path = LiveSessionManager::chunks_dir(&session_id).join(format!("chunk_{:06}.wav", seq));
    let mut recorder = ListenRecorder::with_path(path);
    if let Err(e) = recorder.start() {
        show_status(&ctx.status_label, &format!("Live capture failed: {}", e));
        stop(ctx, runtime);
        return;
    }

    runtime.borrow_mut().recorder = Some(recorder);
    let started_at = Utc::now();
    glib::timeout_add_local_once(Duration::from_secs(chunk_secs), move || {
        finish_chunk(ctx, runtime, seq, started_at);
    });
}

fn finish_chunk(
    ctx: LiveContext,
    runtime: Rc<RefCell<LiveRuntime>>,
    seq: u64,
    started_at: DateTime<Utc>,
) {
    let audio_path = {
        let mut live = runtime.borrow_mut();
        let Some(mut recorder) = live.recorder.take() else {
            return;
        };
        match recorder.stop() {
            Ok(path) => path,
            Err(e) => {
                let active = live.is_active();
                drop(live);
                if active {
                    show_status(&ctx.status_label, &format!("Live chunk skipped: {}", e));
                    start_next_chunk(ctx, runtime);
                }
                return;
            }
        }
    };

    if runtime.borrow().is_active() {
        start_next_chunk(ctx.clone(), runtime.clone());
    }
    process_chunk(ctx, runtime, seq, started_at, audio_path);
}

fn process_chunk(
    ctx: LiveContext,
    runtime: Rc<RefCell<LiveRuntime>>,
    seq: u64,
    started_at: DateTime<Utc>,
    audio_path: PathBuf,
) {
    let (stt_provider, stt_key, stt_model) = {
        let state = ctx.state.borrow();
        (
            state.settings.stt_provider,
            state.settings.active_stt_key().to_string(),
            state.settings.active_stt_model().to_string(),
        )
    };

    ctx.status_label.set_text("Live: transcribing");
    ctx.status_label.set_visible(true);

    glib::spawn_future_local(async move {
        let result = gio::spawn_blocking(move || {
            let vad_config = VadConfig::default();
            if analyze_audio_file(&audio_path, &vad_config).is_some_and(|vad| !vad.has_speech) {
                let _ = fs::remove_file(&audio_path);
                return Ok::<ChunkResult, String>(ChunkResult::Silent);
            }

            let client = WhisperClient::new(stt_provider, &stt_key, &stt_model);
            let result = client.transcribe(&audio_path).map_err(|e| e.to_string());
            let _ = fs::remove_file(&audio_path);
            let text = result?;
            if text.trim().is_empty() {
                Ok(ChunkResult::Silent)
            } else {
                Ok(ChunkResult::Speech(text))
            }
        })
        .await;

        match result {
            Ok(Ok(ChunkResult::Silent)) => handle_silent_chunk(ctx, runtime),
            Ok(Ok(ChunkResult::Speech(text))) => {
                handle_speech_chunk(ctx, runtime, seq, started_at, text)
            }
            Ok(Err(e)) => show_status(&ctx.status_label, &format!("Live STT failed: {}", e)),
            Err(e) => show_status(&ctx.status_label, &format!("Live STT task failed: {:?}", e)),
        }
    });
}

fn handle_speech_chunk(
    ctx: LiveContext,
    runtime: Rc<RefCell<LiveRuntime>>,
    seq: u64,
    started_at: DateTime<Utc>,
    text: String,
) {
    let segment = TranscriptSegment {
        seq,
        started_at,
        ended_at: Utc::now(),
        text,
    };

    {
        let mut live = runtime.borrow_mut();
        let Some(session) = live.session.as_mut() else {
            return;
        };
        session.last_speech_seq = Some(seq);
        session.transcript.append(segment.clone());
        if let Err(e) = LiveSessionManager::append_transcript(&session.metadata.id, &segment) {
            tracing::warn!("Failed to save live transcript: {}", e);
        }
        update_transcript_preview(&ctx, session);
    }

    show_status(&ctx.status_label, "Live: listening");
}

fn handle_silent_chunk(ctx: LiveContext, runtime: Rc<RefCell<LiveRuntime>>) {
    let should_trigger = {
        let live = runtime.borrow();
        let Some(session) = live.session.as_ref() else {
            return;
        };
        ctx.state.borrow().settings.live_auto_suggest
            && session.last_speech_seq.is_some()
            && session.pending_range().is_some()
    };

    if should_trigger {
        request_answer(ctx, runtime, false);
    } else if runtime.borrow().is_active() {
        show_status(&ctx.status_label, "Live: listening");
    }
}

fn request_answer(ctx: LiveContext, runtime: Rc<RefCell<LiveRuntime>>, manual: bool) {
    let (range, transcript_text) = {
        let mut live = runtime.borrow_mut();
        let Some(session) = live.session.as_mut() else {
            show_status(&ctx.status_label, "Live is not active");
            return;
        };
        if session.in_flight.is_some() {
            show_status(&ctx.status_label, "Live: thinking");
            return;
        }
        let Some(range) = session.pending_range() else {
            if manual {
                show_status(&ctx.status_label, "Live: no new transcript");
            }
            return;
        };
        let transcript_text = session.transcript.compact_text(range);
        session.in_flight = Some(range);
        (range, transcript_text)
    };

    let (provider, api_key, model, system_prompt, mut messages) = {
        let state = ctx.state.borrow();
        (
            state.settings.provider,
            state.settings.active_api_key().to_string(),
            state.settings.active_model().to_string(),
            live_system_prompt(&state),
            state.current_messages.clone(),
        )
    };

    let pending_message =
        live_transcript_message(transcript_text, ctx.attached_image.borrow().clone());
    messages.push(pending_message.clone());

    show_status(&ctx.status_label, "Live: thinking");
    glib::spawn_future_local(async move {
        let result = gio::spawn_blocking(move || {
            let client = LlmClient::new(provider, &api_key, &model);
            client
                .chat(&messages, &system_prompt)
                .map_err(|e| e.to_string())
        })
        .await;

        match result {
            Ok(Ok(response)) => apply_live_response(ctx, runtime, range, pending_message, response),
            Ok(Err(e)) => {
                clear_in_flight(&runtime, range);
                show_status(&ctx.status_label, &format!("Live LLM failed: {}", e));
            }
            Err(e) => {
                clear_in_flight(&runtime, range);
                show_status(&ctx.status_label, &format!("Live LLM task failed: {:?}", e));
            }
        }
    });
}

fn apply_live_response(
    ctx: LiveContext,
    runtime: Rc<RefCell<LiveRuntime>>,
    range: LiveRequestRange,
    pending_message: Message,
    response: String,
) {
    let mut reschedule = false;
    {
        let mut live = runtime.borrow_mut();
        let Some(session) = live.session.as_mut() else {
            return;
        };
        if session.transcript.max_seq() > range.to_seq {
            session.in_flight = None;
            reschedule = session.pending_range().is_some();
        } else if is_no_suggestion(&response) {
            session.mark_evaluated(range);
            show_status(&ctx.status_label, "Live: no suggestion");
        } else {
            let mut state = ctx.state.borrow_mut();
            state.current_messages.push(pending_message);
            state.current_messages.push(Message {
                role: Role::Assistant,
                content: response,
                image: None,
            });
            session.mark_evaluated(range);
            messaging::update_chat_display(&ctx.chat_view, &state);
            show_status(&ctx.status_label, "Live: answered");
        }
    }

    if reschedule && runtime.borrow().is_active() {
        request_answer(ctx, runtime, false);
    }
}

fn clear_in_flight(runtime: &Rc<RefCell<LiveRuntime>>, range: LiveRequestRange) {
    if let Some(session) = runtime.borrow_mut().session.as_mut() {
        if session.in_flight == Some(range) {
            session.in_flight = None;
        }
    }
}

fn live_system_prompt(state: &AppState) -> String {
    let active_prompt = state.active_prompt();
    if active_prompt.trim().is_empty() {
        LIVE_SYSTEM_PROMPT.to_string()
    } else {
        format!("{}\n\n{}", LIVE_SYSTEM_PROMPT, active_prompt)
    }
}

fn update_transcript_preview(ctx: &LiveContext, session: &LiveSession) {
    if !ctx.state.borrow().settings.live_debug_transcript {
        return;
    }
    let text = session.transcript.latest_lines(5);
    ctx.transcript_label
        .set_text(&format!("Live transcript:\n{}", text));
    ctx.transcript_label.set_visible(!text.is_empty());
}

fn set_live_buttons(ctx: &LiveContext, active: bool) {
    ctx.live_btn
        .set_label(if active { "⏹\u{fe0f}" } else { "📡" });
    ctx.reply_btn.set_sensitive(active);
    ctx.audio_btn.set_sensitive(!active);
    ctx.listen_btn.set_sensitive(!active);
}

fn show_status(status_label: &Label, text: &str) {
    status_label.set_text(text);
    status_label.set_visible(true);
}
