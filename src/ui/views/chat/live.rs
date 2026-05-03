// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Chat-integrated Live Assistant controls.

use super::messaging;
use crate::core::api::{ChatOptions, LlmClient, WhisperClient};
use crate::core::audio::{analyze_audio_file, AudioRecorder, ListenRecorder, VadConfig};
use crate::core::live::{
    is_no_suggestion, live_transcript_message, LiveRequestRange, LiveSession, LiveSessionManager,
    LIVE_SYSTEM_PROMPT, MAX_PENDING_CHUNKS,
};
use crate::core::state::{AiProvider, AppState, ImageData, Message, Role};
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

const LIVE_CONTEXT_MESSAGES: usize = 8;

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
    active_chunk: Option<ActiveChunk>,
    next_seq: u64,
}

#[derive(Clone, Copy)]
struct ActiveChunk {
    seq: u64,
    started_at: DateTime<Utc>,
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
    force_flush_and_answer(ctx, runtime);
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
            state.settings.effective_live_chunk_secs(),
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
        live.active_chunk = None;
        let fallback_seq = live.next_seq.saturating_sub(1);
        if let Some(mut session) = live.session.take() {
            let closing_seq = session
                .pending_to_seq
                .or_else(|| (fallback_seq > 0).then_some(fallback_seq));
            close_pending_session(&ctx, &mut session, closing_seq);
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

    let started_at = Utc::now();
    {
        let mut live = runtime.borrow_mut();
        live.recorder = Some(recorder);
        live.active_chunk = Some(ActiveChunk { seq, started_at });
    }
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
        if live.active_chunk.is_none_or(|chunk| chunk.seq != seq) {
            return;
        }
        live.active_chunk = None;
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
    process_chunk(ctx, runtime, seq, started_at, audio_path, false);
}

fn force_flush_and_answer(ctx: LiveContext, runtime: Rc<RefCell<LiveRuntime>>) {
    let (recorder, chunk) = {
        let mut live = runtime.borrow_mut();
        if live.session.is_none() {
            show_status(&ctx.status_label, "Live is not active");
            return;
        }
        if live
            .session
            .as_ref()
            .is_some_and(|session| session.in_flight.is_some())
        {
            show_status(&ctx.status_label, "Live: thinking");
            return;
        }
        (live.recorder.take(), live.active_chunk.take())
    };

    let (Some(mut recorder), Some(chunk)) = (recorder, chunk) else {
        finish_force_reply(ctx, runtime, None);
        return;
    };

    let stopped = recorder.stop();

    match stopped {
        Ok(audio_path) => {
            process_chunk(ctx, runtime, chunk.seq, chunk.started_at, audio_path, true);
        }
        Err(e) => {
            show_status(&ctx.status_label, &format!("Live flush skipped: {}", e));
            finish_force_reply(ctx, runtime, Some(chunk.seq));
        }
    }
}

fn process_chunk(
    ctx: LiveContext,
    runtime: Rc<RefCell<LiveRuntime>>,
    seq: u64,
    started_at: DateTime<Utc>,
    audio_path: PathBuf,
    force_reply_after: bool,
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
            Ok(Ok(ChunkResult::Silent)) => {
                if force_reply_after {
                    finish_force_reply(ctx, runtime, Some(seq));
                } else {
                    handle_silent_chunk(ctx, runtime, seq);
                }
            }
            Ok(Ok(ChunkResult::Speech(text))) => {
                handle_speech_chunk(ctx.clone(), runtime.clone(), seq, started_at, text);
                if force_reply_after {
                    finish_force_reply(ctx, runtime, Some(seq));
                }
            }
            Ok(Err(e)) => {
                show_status(&ctx.status_label, &format!("Live STT failed: {}", e));
                if force_reply_after {
                    finish_force_reply(ctx, runtime, Some(seq));
                }
            }
            Err(e) => {
                show_status(&ctx.status_label, &format!("Live STT task failed: {:?}", e));
                if force_reply_after {
                    finish_force_reply(ctx, runtime, Some(seq));
                }
            }
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
    let should_close = {
        let mut live = runtime.borrow_mut();
        let Some(session) = live.session.as_mut() else {
            return;
        };
        session.last_speech_seq = Some(seq);
        session.append_to_pending(seq, started_at, Utc::now(), &text);
        update_transcript_preview(&ctx, session);
        session
            .pending_span()
            .is_some_and(|span| span >= MAX_PENDING_CHUNKS)
    };

    if should_close {
        close_pending_utterance(&ctx, &runtime, Some(seq));
    }

    show_status(&ctx.status_label, "Live: listening");
}

fn handle_silent_chunk(ctx: LiveContext, runtime: Rc<RefCell<LiveRuntime>>, seq: u64) {
    let closed = close_pending_utterance(&ctx, &runtime, Some(seq));
    let should_trigger = closed && ctx.state.borrow().settings.live_auto_suggest;

    if should_trigger {
        request_answer(ctx, runtime, false);
    } else if runtime.borrow().is_active() {
        show_status(&ctx.status_label, "Live: listening");
    }
}

fn finish_force_reply(
    ctx: LiveContext,
    runtime: Rc<RefCell<LiveRuntime>>,
    closing_seq: Option<u64>,
) {
    if !runtime.borrow().is_active() {
        return;
    }
    let closed = close_pending_utterance(&ctx, &runtime, closing_seq);
    let has_closed_transcript = runtime
        .borrow()
        .session
        .as_ref()
        .is_some_and(|session| session.pending_range().is_some());
    if closed || has_closed_transcript {
        request_answer(ctx.clone(), runtime.clone(), true);
    } else {
        show_status(&ctx.status_label, "Live: no new transcript");
    }
    if runtime.borrow().is_active() {
        start_next_chunk(ctx, runtime);
    }
}

fn close_pending_utterance(
    ctx: &LiveContext,
    runtime: &Rc<RefCell<LiveRuntime>>,
    closing_seq: Option<u64>,
) -> bool {
    let mut live = runtime.borrow_mut();
    let Some(session) = live.session.as_mut() else {
        return false;
    };
    close_pending_session(ctx, session, closing_seq)
}

fn close_pending_session(
    ctx: &LiveContext,
    session: &mut LiveSession,
    closing_seq: Option<u64>,
) -> bool {
    let from_seq = session.pending_from_seq;
    let to_seq = session.pending_to_seq;
    let Some(closing_seq) = closing_seq.or(session.pending_to_seq) else {
        return false;
    };
    let Some(segment) = session.take_pending_segment(closing_seq) else {
        return false;
    };

    session.transcript.append(segment.clone());
    if let Err(e) = LiveSessionManager::append_transcript(&session.metadata.id, &segment) {
        tracing::warn!("Failed to save live transcript: {}", e);
    }
    tracing::info!(
        "closed utterance seq={} chunks={}-{} len={}",
        segment.seq,
        from_seq.unwrap_or(segment.seq),
        to_seq.unwrap_or(segment.seq),
        segment.text.len()
    );
    update_transcript_preview(ctx, session);
    true
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

    let (provider, api_key, model, system_prompt, mut messages, cache_up_to_message) = {
        let state = ctx.state.borrow();
        let provider = state.settings.provider;
        let messages = live_context_messages(&state, provider);
        let cache_up_to_message = (provider == AiProvider::Anthropic)
            .then(|| last_anthropic_message_index(&messages))
            .flatten();
        (
            provider,
            state.settings.active_api_key().to_string(),
            state.settings.active_model().to_string(),
            live_system_prompt(&state),
            messages,
            cache_up_to_message,
        )
    };

    let pending_message =
        live_transcript_message(transcript_text, ctx.attached_image.borrow().clone());
    messages.push(pending_message.clone());
    let options = ChatOptions {
        anthropic_cache: provider == AiProvider::Anthropic,
        cache_up_to_message,
    };

    show_status(&ctx.status_label, "Live: thinking");
    glib::spawn_future_local(async move {
        let result = gio::spawn_blocking(move || {
            let client = LlmClient::new(provider, &api_key, &model);
            client
                .chat_with_options(&messages, &system_prompt, options)
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

fn live_context_messages(state: &AppState, provider: AiProvider) -> Vec<Message> {
    if provider == AiProvider::Anthropic {
        return state.current_messages.clone();
    }

    let len = state.current_messages.len();
    state.current_messages[len.saturating_sub(LIVE_CONTEXT_MESSAGES)..].to_vec()
}

fn last_anthropic_message_index(messages: &[Message]) -> Option<usize> {
    messages
        .iter()
        .filter(|msg| msg.role != Role::System)
        .enumerate()
        .last()
        .map(|(idx, _)| idx)
}

fn update_transcript_preview(ctx: &LiveContext, session: &LiveSession) {
    if !ctx.state.borrow().settings.live_debug_transcript {
        return;
    }
    let mut lines = Vec::new();
    let closed = session.transcript.latest_lines(3);
    if !closed.is_empty() {
        lines.push(closed);
    }
    if session.has_pending() {
        let pending_text = session
            .pending_utterance
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        lines.push(format!(
            "pending [{}-{}]: {}",
            session.pending_from_seq.unwrap_or(0),
            session.pending_to_seq.unwrap_or(0),
            pending_text
        ));
    }
    let text = lines.join("\n");
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

#[cfg(test)]
mod tests {
    use super::*;

    fn message(role: Role) -> Message {
        Message {
            role,
            content: String::new(),
            image: None,
        }
    }

    #[test]
    fn anthropic_cache_index_ignores_system_messages() {
        let messages = vec![
            message(Role::System),
            message(Role::User),
            message(Role::Assistant),
        ];

        assert_eq!(last_anthropic_message_index(&messages), Some(1));
    }

    #[test]
    fn anthropic_cache_index_is_none_without_chat_messages() {
        let messages = vec![message(Role::System)];

        assert_eq!(last_anthropic_message_index(&messages), None);
    }
}
