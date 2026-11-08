// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Meeting recorder — orchestrates dual audio recording (system + optional mic)
//! and post-recording WAV-to-OPUS conversion.

use super::capture::AudioRecorder;
use super::listen::ListenRecorder;
use crate::core::meeting::{Meeting, MeetingManager, MeetingStatus};
use std::path::Path;
use std::process::{Command, Stdio};

use crate::core::utils::command_exists;

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
        let mut meeting = Meeting::new(record_mic);
        let meeting_dir = MeetingManager::prepare_meeting_dir(&meeting.id)
            .map_err(|e| format!("Failed to create meeting directory: {}", e))?;

        // System audio recorder (monitor source)
        let mut system_recorder = ListenRecorder::new();
        system_recorder.start(meeting_dir.join("system.wav"))?;

        // Optional mic recorder
        let mic_recorder = if record_mic {
            let mut mic = AudioRecorder::new();
            match mic.start(meeting_dir.join("mic.wav")) {
                Ok(()) => Some(mic),
                Err(e) => {
                    tracing::warn!("Failed to start mic recording: {}", e);
                    meeting.has_mic = false;
                    meeting.last_error = Some(format!("Microphone not recorded: {}", e));
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
    /// A capture failure is recorded on the meeting so the user sees it.
    pub fn stop(&mut self) -> Result<Meeting, String> {
        let mut problems: Vec<String> = self.meeting.last_error.take().into_iter().collect();
        if let Err(e) = self.system_recorder.stop() {
            problems.push(format!("System audio: {}", e));
        }
        if let Some(ref mut mic) = self.mic_recorder {
            if let Err(e) = mic.stop() {
                problems.push(format!("Microphone: {}", e));
            }
        }

        // Update meeting metadata
        self.meeting.last_error = (!problems.is_empty()).then(|| problems.join("; "));
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

    /// True when system audio capture died while the meeting is still running.
    pub fn capture_failed(&mut self) -> bool {
        self.system_recorder.has_exited()
    }
}

impl Drop for MeetingRecorder {
    fn drop(&mut self) {
        if self.is_recording() {
            let _ = self.stop();
        }
    }
}

/// Create OPUS copies for playback and long-term storage.
/// Source WAVs stay until transcription succeeds: providers take WAV and can
/// reject Opus, and only WAV can be split to fit the upload limit.
/// Returns true if at least one file was converted.
pub fn convert_meeting_to_opus(meeting_id: &str) -> bool {
    let dir = MeetingManager::meeting_dir(meeting_id);
    convert_wav_to_opus(&dir)
}

fn convert_wav_to_opus(dir: &Path) -> bool {
    let converter = find_converter();
    let converter = match converter {
        Some(c) => c,
        None => {
            tracing::warn!("No OPUS encoder found (install opus-tools or ffmpeg)");
            return false;
        }
    };

    // Build a mixed track for playback when both sources exist.
    let system_wav = dir.join("system.wav");
    let mic_wav = dir.join("mic.wav");
    let mixed_wav = dir.join("mixed.wav");
    if system_wav.exists() && mic_wav.exists() {
        match run_ffmpeg_mix(&system_wav, &mic_wav, &mixed_wav) {
            Ok(()) => tracing::info!("Created mixed.wav from system + mic"),
            Err(e) => tracing::warn!("Failed to create mixed.wav: {}", e),
        }
    }

    let mut any_converted = false;

    for name in &["system.wav", "mic.wav", "mixed.wav"] {
        let wav_path = dir.join(name);
        if !wav_path.exists() {
            continue;
        }

        let opus_path = dir.join(name.replace(".wav", ".opus"));
        let result = encode_atomically(&opus_path, |partial| match converter {
            Converter::Opusenc => run_opusenc(&wav_path, partial),
            Converter::Ffmpeg => run_ffmpeg(&wav_path, partial),
        });

        match result {
            Ok(()) => {
                // Only the playback mix is safe to drop, transcription reads the sources.
                if *name == "mixed.wav" {
                    let _ = std::fs::remove_file(&wav_path);
                }
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

/// Encode under a temporary name, so a file at `opus` is always complete:
/// after transcription its presence allows deleting the source WAV.
fn encode_atomically(
    opus: &Path,
    encode: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
    let partial = opus.with_extension("partial.opus");
    let result = encode(&partial).and_then(|()| {
        std::fs::rename(&partial, opus)
            .map_err(|e| format!("Failed to save {}: {}", opus.display(), e))
    });
    if result.is_err() {
        let _ = std::fs::remove_file(&partial);
    }
    result
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

fn run_ffmpeg_mix(system_wav: &Path, mic_wav: &Path, mixed_wav: &Path) -> Result<(), String> {
    if !command_exists("ffmpeg") {
        return Err("ffmpeg not found".to_string());
    }

    let status = Command::new("ffmpeg")
        .args([
            "-i",
            &system_wav.to_string_lossy(),
            "-i",
            &mic_wav.to_string_lossy(),
            "-filter_complex",
            "amix=inputs=2:duration=longest:dropout_transition=0",
            "-c:a",
            "pcm_s16le",
            "-ar",
            "16000",
            "-ac",
            "1",
            "-y",
            &mixed_wav.to_string_lossy(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("ffmpeg mix failed: {}", e))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("ffmpeg mix exited with {}", status))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn failed_encoder_leaves_no_opus_file() {
        let dir = tempfile::tempdir().unwrap();
        let opus = dir.path().join("system.opus");

        let result = encode_atomically(&opus, |partial| {
            fs::write(partial, "truncated").unwrap();
            Err("encoder crashed".to_string())
        });

        assert!(result.is_err());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn finished_encoding_appears_under_the_final_name() {
        let dir = tempfile::tempdir().unwrap();
        let opus = dir.path().join("system.opus");

        encode_atomically(&opus, |partial| {
            assert!(!opus.exists());
            fs::write(partial, "audio").map_err(|e| e.to_string())
        })
        .unwrap();

        assert_eq!(fs::read_to_string(&opus).unwrap(), "audio");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
