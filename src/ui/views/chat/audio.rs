// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Audio handlers - recording and transcription

use crate::core::api::WhisperClient;
use crate::core::audio::{analyze_audio_file, AudioRecorder, ListenRecorder, VadConfig};
use crate::core::state::AppState;
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Button, Entry, Label};
use std::cell::RefCell;
use std::rc::Rc;

/// Handle microphone recording button click
pub fn handle_audio_click(
    recorder: &Rc<RefCell<AudioRecorder>>,
    state: &Rc<RefCell<AppState>>,
    status_label: &Label,
    audio_btn: &Button,
    entry: &Entry,
) {
    if recorder.borrow().is_recording() {
        audio_btn.set_icon_name("audio-input-microphone-symbolic");
        audio_btn.remove_css_class("destructive-action");

        let audio_path = match recorder.borrow_mut().stop() {
            Ok(path) => path,
            Err(e) => {
                status_label.set_text(&format!("❌ {}", e));
                status_label.set_visible(true);
                return;
            }
        };

        transcribe_and_insert(audio_path, state, status_label, entry, None);
    } else {
        match recorder.borrow_mut().start() {
            Ok(()) => {
                audio_btn.set_icon_name("media-playback-stop-symbolic");
                audio_btn.add_css_class("destructive-action");
                status_label.set_text("Recording...");
                status_label.set_visible(true);
            }
            Err(e) => {
                status_label.set_text(&format!("❌ {}", e));
                status_label.set_visible(true);
            }
        }
    }
}

/// Handle system audio capture button click
pub fn handle_listen_click(
    listener: &Rc<RefCell<ListenRecorder>>,
    state: &Rc<RefCell<AppState>>,
    status_label: &Label,
    listen_btn: &Button,
    entry: &Entry,
) {
    if listener.borrow().is_listening() {
        listen_btn.set_icon_name("audio-volume-high-symbolic");
        listen_btn.remove_css_class("destructive-action");

        let audio_path = match listener.borrow_mut().stop() {
            Ok(path) => path,
            Err(e) => {
                status_label.set_text(&format!("❌ {}", e));
                status_label.set_visible(true);
                return;
            }
        };

        transcribe_and_insert(audio_path, state, status_label, entry, Some("🎧 [Heard]: "));
    } else {
        match listener.borrow_mut().start() {
            Ok(()) => {
                listen_btn.set_icon_name("media-playback-stop-symbolic");
                listen_btn.add_css_class("destructive-action");
                status_label.set_text("Listening...");
                status_label.set_visible(true);
            }
            Err(e) => {
                status_label.set_text(&format!("❌ {}", e));
                status_label.set_visible(true);
            }
        }
    }
}

/// Transcribe audio and insert result into entry field
/// Uses VAD to skip transcription if no speech detected
fn transcribe_and_insert(
    audio_path: std::path::PathBuf,
    state: &Rc<RefCell<AppState>>,
    status_label: &Label,
    entry: &Entry,
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
        status_label.set_text("⚠️ Set STT API key in Settings");
        status_label.set_visible(true);
        return;
    }

    // VAD check: skip transcription if no speech detected
    let vad_config = VadConfig::default();
    if let Some(vad_result) = analyze_audio_file(&audio_path, &vad_config) {
        if !vad_result.has_speech {
            tracing::info!(
                "VAD: No speech detected (rms={:.4}, peak={:.4})",
                vad_result.avg_rms,
                vad_result.peak
            );
            status_label.set_text("🔇 No speech detected");
            status_label.set_visible(true);
            // Auto-hide after 2 seconds
            let status_c = status_label.clone();
            glib::timeout_add_local_once(std::time::Duration::from_secs(2), move || {
                status_c.set_visible(false);
            });
            return;
        }
        tracing::info!(
            "VAD: Speech detected (rms={:.4}, peak={:.4}, duration={:.1}s)",
            vad_result.avg_rms,
            vad_result.peak,
            vad_result.duration_secs
        );
    }

    status_label.set_text("⏳ Transcribing...");
    status_label.set_visible(true);

    let status_c = status_label.clone();
    let entry_c = entry.clone();
    let prefix_owned = prefix.map(|s| s.to_string());

    glib::spawn_future_local(async move {
        let result = gio::spawn_blocking(move || {
            let client = WhisperClient::new(stt_provider, &stt_key, &stt_model);
            client.transcribe(&audio_path).map_err(|e| e.to_string())
        })
        .await;

        match result {
            Ok(Ok(text)) => {
                let formatted = match &prefix_owned {
                    Some(p) => format!("{}{}", p, text),
                    None => text,
                };
                let current = entry_c.text();
                let new_text = if current.is_empty() {
                    formatted
                } else {
                    format!("{}\n{}", current, formatted)
                };
                entry_c.set_text(&new_text);
                status_c.set_visible(false);
            }
            Ok(Err(e)) => {
                status_c.set_text(&format!("❌ {}", e));
            }
            Err(e) => {
                status_c.set_text(&format!("❌ Transcription failed: {:?}", e));
            }
        }
    });
}
