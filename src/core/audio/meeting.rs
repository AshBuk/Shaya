// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Meeting recorder — orchestrates dual audio recording (system + optional mic)
//! and post-recording WAV-to-OPUS conversion.

use super::capture::AudioRecorder;
use super::listen::ListenRecorder;
use crate::core::meeting::{Meeting, MeetingManager, MeetingStatus};
use std::path::Path;
use std::process::{Command, Stdio};

use super::command_exists;

/// Orchestrates system audio + optional mic recording for meetings
pub struct MeetingRecorder {
    meeting: Meeting,
    system_recorder: ListenRecorder,
    mic_recorder: Option<AudioRecorder>,
    started_at: std::time::Instant,
}

impl MeetingRecorder {
    /// Start a new meeting recording
    pub fn start(record_mic: bool) -> Result<Self, String> {
        let meeting = Meeting::new(record_mic);
        let meeting_dir = MeetingManager::meeting_dir(&meeting.id);

        std::fs::create_dir_all(&meeting_dir)
            .map_err(|e| format!("Failed to create meeting directory: {}", e))?;

        // System audio recorder (monitor source)
        let system_path = meeting_dir.join("system.wav");
        let mut system_recorder = ListenRecorder::with_path(system_path);
        system_recorder.start()?;

        // Optional mic recorder
        let mic_recorder = if record_mic {
            let mic_path = meeting_dir.join("mic.wav");
            let mut mic = AudioRecorder::with_path(mic_path);
            match mic.start() {
                Ok(()) => Some(mic),
                Err(e) => {
                    tracing::warn!("Failed to start mic recording: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // Save initial metadata
        MeetingManager::save(&meeting)
            .map_err(|e| format!("Failed to save meeting metadata: {}", e))?;

        tracing::info!("Meeting started: {} (mic: {})", meeting.id, record_mic);

        Ok(Self {
            meeting,
            system_recorder,
            mic_recorder,
            started_at: std::time::Instant::now(),
        })
    }

    /// Stop recording and finalize meeting.
    /// WAV files are kept for transcription; call `convert_meeting_to_opus()` after.
    pub fn stop(&mut self) -> Result<Meeting, String> {
        // Stop recorders
        let _ = self.system_recorder.stop();
        if let Some(ref mut mic) = self.mic_recorder {
            let _ = mic.stop();
        }

        // Update meeting metadata
        self.meeting.duration_secs = self.started_at.elapsed().as_secs();
        self.meeting.status = MeetingStatus::Completed;

        MeetingManager::save(&self.meeting)
            .map_err(|e| format!("Failed to save meeting: {}", e))?;

        tracing::info!(
            "Meeting stopped: {} ({}s)",
            self.meeting.id,
            self.meeting.duration_secs
        );

        Ok(self.meeting.clone())
    }

    /// Check if recording is active
    pub fn is_recording(&self) -> bool {
        self.system_recorder.is_listening()
    }

    /// Get elapsed recording time in seconds
    pub fn elapsed_secs(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }
}

impl Drop for MeetingRecorder {
    fn drop(&mut self) {
        if self.is_recording() {
            let _ = self.stop();
        }
    }
}

/// Convert all WAV files in a meeting directory to OPUS, deleting originals on success.
/// Call this AFTER transcription is complete (transcription needs the WAV files).
/// Returns true if at least one file was converted.
pub fn convert_meeting_to_opus(meeting_id: &str) -> bool {
    let dir = MeetingManager::meeting_dir(meeting_id);
    convert_wav_to_opus(&dir)
}

/// Convert all WAV files in a directory to OPUS, deleting originals on success.
fn convert_wav_to_opus(dir: &Path) -> bool {
    let converter = find_converter();
    let converter = match converter {
        Some(c) => c,
        None => {
            tracing::warn!("No OPUS encoder found (install opus-tools or ffmpeg)");
            return false;
        }
    };

    let mut any_converted = false;

    for name in &["system.wav", "mic.wav"] {
        let wav_path = dir.join(name);
        if !wav_path.exists() {
            continue;
        }

        let opus_path = dir.join(name.replace(".wav", ".opus"));
        let result = match converter {
            Converter::Opusenc => run_opusenc(&wav_path, &opus_path),
            Converter::Ffmpeg => run_ffmpeg(&wav_path, &opus_path),
        };

        match result {
            Ok(()) => {
                // Delete WAV after successful conversion
                let _ = std::fs::remove_file(&wav_path);
                tracing::info!("Converted {} to OPUS", name);
                any_converted = true;
            }
            Err(e) => {
                tracing::warn!("Failed to convert {} to OPUS: {}", name, e);
                // Keep WAV file on failure
            }
        }
    }

    any_converted
}

#[derive(Clone, Copy)]
enum Converter {
    Opusenc,
    Ffmpeg,
}

fn find_converter() -> Option<Converter> {
    if command_exists("opusenc") {
        Some(Converter::Opusenc)
    } else if command_exists("ffmpeg") {
        Some(Converter::Ffmpeg)
    } else {
        None
    }
}

fn run_opusenc(wav: &Path, opus: &Path) -> Result<(), String> {
    let status = Command::new("opusenc")
        .args([
            "--bitrate",
            "32",
            "--speech",
            &wav.to_string_lossy(),
            &opus.to_string_lossy(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("opusenc failed: {}", e))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("opusenc exited with {}", status))
    }
}

fn run_ffmpeg(wav: &Path, opus: &Path) -> Result<(), String> {
    let status = Command::new("ffmpeg")
        .args([
            "-i",
            &wav.to_string_lossy(),
            "-c:a",
            "libopus",
            "-b:a",
            "32k",
            "-application",
            "voip",
            "-y",
            &opus.to_string_lossy(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("ffmpeg failed: {}", e))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("ffmpeg exited with {}", status))
    }
}
