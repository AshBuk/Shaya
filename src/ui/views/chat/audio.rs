// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Audio handlers - recording and transcription

use super::messaging::input_text;
use super::status::StatusBar;
use super::task::{self, ChatScope};
use crate::core::api::WhisperClient;
use crate::core::audio::{analyze_audio_file, AudioRecorder, ListenRecorder, VadConfig};
use crate::core::state::AppState;
use crate::core::utils;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Button, TextBuffer};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// A finished recording on its way into the input of the chat it was made in.
/// The file is removed once dropped. A failed transcription keeps it for Retry,
/// and a dismissed error drops it.
struct Dictation {
    path: PathBuf,
    scope: ChatScope,
    prefix: Option<&'static str>,
}

impl Dictation {
    fn new(path: PathBuf, state: &AppState, prefix: Option<&'static str>) -> Self {
        Self {
            path,
            scope: ChatScope::current(state),
            prefix,
        }
    }
}

impl Drop for Dictation {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Handle microphone recording button click
pub fn handle_audio_click(
    recorder: &Rc<RefCell<AudioRecorder>>,
    state: &Rc<RefCell<AppState>>,
    status: &StatusBar,
    audio_btn: &Button,
    input: &TextBuffer,
) {
    if recorder.borrow().is_recording() {
        audio_btn.remove_css_class("recording");
        status.stop_activity(audio_btn);

        let audio_path = match recorder.borrow_mut().stop() {
            Ok(path) => path,
            Err(e) => {
                status.error(&format!("{e:#}"));
                return;
            }
        };

        let dictation = Dictation::new(audio_path, &state.borrow(), None);
        transcribe_and_insert(dictation, state, status, input);
    } else {
        // A new file per recording: the previous one may still be uploading.
        let started = utils::temp_path("voice", "wav")
            .map_err(Into::into)
            .and_then(|path| recorder.borrow_mut().start(path));
        match started {
            Ok(()) => {
                audio_btn.add_css_class("recording");
                status.start_activity("Recording microphone", audio_btn, false);
            }
            Err(e) => status.error(&format!("{e:#}")),
        }
    }
}

/// Handle system audio capture button click
pub fn handle_listen_click(
    listener: &Rc<RefCell<ListenRecorder>>,
    state: &Rc<RefCell<AppState>>,
    status: &StatusBar,
    listen_btn: &Button,
    input: &TextBuffer,
) {
    if listener.borrow().is_listening() {
        listen_btn.remove_css_class("recording");
        status.stop_activity(listen_btn);

        let audio_path = match listener.borrow_mut().stop() {
            Ok(path) => path,
            Err(e) => {
                status.error(&format!("{e:#}"));
                return;
            }
        };

        let dictation = Dictation::new(audio_path, &state.borrow(), Some("🎧 [Heard]: "));
        transcribe_and_insert(dictation, state, status, input);
    } else {
        let started = utils::temp_path("listen", "wav")
            .map_err(Into::into)
            .and_then(|path| listener.borrow_mut().start(path));
        match started {
            Ok(()) => {
                listen_btn.add_css_class("recording");
                status.start_activity("Capturing system audio", listen_btn, false);
            }
            Err(e) => status.error(&format!("{e:#}")),
        }
    }
}

/// Transcribe a dictation and append it to the input.
fn transcribe_and_insert(
    dictation: Dictation,
    state: &Rc<RefCell<AppState>>,
    status: &StatusBar,
    input: &TextBuffer,
) {
    let (stt_provider, stt_key, stt_model) = {
        let s = state.borrow();
        (
            s.settings.stt_provider,
            s.settings.active_stt_key().to_string(),
            s.settings.active_stt_model().to_string(),
        )
    };
    if stt_key.is_empty() {
        offer_retry(
            dictation,
            state,
            status,
            input,
            "Set STT API key in Settings",
        );
        return;
    }

    status.busy("Transcribing…");

    let state = state.clone();
    let status = status.clone();
    let input = input.clone();
    let audio_path = dictation.path.clone();

    glib::spawn_future_local(async move {
        let result = task::run(&state, dictation.scope, move || {
            if !has_speech(&audio_path) {
                return Ok(None);
            }
            WhisperClient::new(stt_provider, &stt_key, &stt_model)
                .transcribe(&audio_path)
                .map(Some)
        })
        .await;

        match result {
            None => status.flash("Transcription cancelled: the chat changed"),
            Some(Ok(None)) => status.flash("No speech detected"),
            Some(Ok(Some(text))) => {
                let formatted = match dictation.prefix {
                    Some(p) => format!("{p}{text}"),
                    None => text,
                };
                let current = input_text(&input);
                input.set_text(&if current.is_empty() {
                    formatted
                } else {
                    format!("{current}\n{formatted}")
                });
                status.clear();
            }
            Some(Err(e)) => offer_retry(dictation, &state, &status, &input, &format!("{e:#}")),
        }
    });
}

/// Voice activity check, run on the worker thread: a long WAV takes a while to read.
/// An unreadable file counts as speech and goes to STT.
fn has_speech(path: &Path) -> bool {
    let Some(vad) = analyze_audio_file(path, &VadConfig::default()) else {
        return true;
    };
    tracing::info!(
        "VAD: speech={} (rms={:.4}, peak={:.4}, duration={:.1}s)",
        vad.has_speech,
        vad.avg_rms,
        vad.peak,
        vad.duration_secs
    );
    vad.has_speech
}

/// Show the error with a Retry that transcribes the same recording again,
/// for the chat it was made in.
fn offer_retry(
    dictation: Dictation,
    state: &Rc<RefCell<AppState>>,
    status: &StatusBar,
    input: &TextBuffer,
    error: &str,
) {
    let (state_c, status_c, input_c) = (state.clone(), status.clone(), input.clone());
    status.error_with_retry(error, move || {
        transcribe_and_insert(dictation, &state_c, &status_c, &input_c);
    });
}
