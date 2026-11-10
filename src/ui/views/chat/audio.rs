// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Audio handlers - recording and transcription

use super::messaging::input_text;
use super::status::StatusBar;
use crate::core::api::WhisperClient;
use crate::core::audio::{analyze_audio_file, AudioRecorder, ListenRecorder, VadConfig};
use crate::core::state::AppState;
use crate::core::utils;
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Button, TextBuffer};
use std::cell::RefCell;
use std::rc::Rc;

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
                status.error(&e.to_string());
                return;
            }
        };

        transcribe_and_insert(audio_path, state, status, input, None);
    } else {
        // A new file per recording: the previous one may still be uploading.
        let started = utils::temp_path("voice", "wav")
            .map_err(|e| e.to_string())
            .and_then(|path| recorder.borrow_mut().start(path));
        match started {
            Ok(()) => {
                audio_btn.add_css_class("recording");
                status.start_activity("Recording microphone", audio_btn, false);
            }
            Err(e) => status.error(&e),
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
                status.error(&e.to_string());
                return;
            }
        };

        transcribe_and_insert(audio_path, state, status, input, Some("🎧 [Heard]: "));
    } else {
        let started = utils::temp_path("listen", "wav")
            .map_err(|e| e.to_string())
            .and_then(|path| listener.borrow_mut().start(path));
        match started {
            Ok(()) => {
                listen_btn.add_css_class("recording");
                status.start_activity("Capturing system audio", listen_btn, false);
            }
            Err(e) => status.error(&e),
        }
    }
}

/// Transcribe audio and insert result into the input
/// Uses VAD to skip transcription if no speech detected
fn transcribe_and_insert(
    audio_path: std::path::PathBuf,
    state: &Rc<RefCell<AppState>>,
    status: &StatusBar,
    input: &TextBuffer,
    prefix: Option<&str>,
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
        let _ = std::fs::remove_file(&audio_path);
        status.error("Set STT API key in Settings");
        return;
    }

    // VAD check: skip transcription if no speech detected
    let vad_config = VadConfig::default();
    if let Some(vad_result) = analyze_audio_file(&audio_path, &vad_config) {
        if !vad_result.has_speech {
            let _ = std::fs::remove_file(&audio_path);
            tracing::info!(
                "VAD: No speech detected (rms={:.4}, peak={:.4})",
                vad_result.avg_rms,
                vad_result.peak
            );
            status.flash("No speech detected");
            return;
        }
        tracing::info!(
            "VAD: Speech detected (rms={:.4}, peak={:.4}, duration={:.1}s)",
            vad_result.avg_rms,
            vad_result.peak,
            vad_result.duration_secs
        );
    }

    status.busy("Transcribing…");

    let status = status.clone();
    let input = input.clone();
    let prefix_owned = prefix.map(|s| s.to_string());

    glib::spawn_future_local(async move {
        let result = gio::spawn_blocking(move || {
            let client = WhisperClient::new(stt_provider, &stt_key, &stt_model);
            let result = client.transcribe(&audio_path).map_err(|e| e.to_string());
            let _ = std::fs::remove_file(&audio_path);
            result
        })
        .await;

        match result {
            Ok(Ok(text)) => {
                let formatted = match &prefix_owned {
                    Some(p) => format!("{}{}", p, text),
                    None => text,
                };
                let current = input_text(&input);
                let new_text = if current.is_empty() {
                    formatted
                } else {
                    format!("{}\n{}", current, formatted)
                };
                input.set_text(&new_text);
                status.clear();
            }
            Ok(Err(e)) => status.error(&e),
            Err(e) => status.error(&format!("Transcription failed: {:?}", e)),
        }
    });
}
