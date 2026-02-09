// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Audio capture using system tools (pw-record/parecord)

use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Audio recorder using PipeWire or PulseAudio
pub struct AudioRecorder {
    process: Option<Child>,
    output_path: PathBuf,
    is_recording: Arc<AtomicBool>,
}

impl AudioRecorder {
    /// Create a new audio recorder (default temp path)
    pub fn new() -> Self {
        Self::with_path(std::env::temp_dir().join("shyquiet_recording.wav"))
    }

    /// Create a new audio recorder with custom output path
    pub fn with_path(output_path: PathBuf) -> Self {
        Self {
            process: None,
            output_path,
            is_recording: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check if recording is in progress
    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }

    /// Start recording audio
    pub fn start(&mut self) -> Result<(), String> {
        if self.is_recording() {
            return Err("Already recording".to_string());
        }

        // Remove old recording
        let _ = fs::remove_file(&self.output_path);

        // Try pw-record first (PipeWire), fallback to parecord (PulseAudio)
        let (cmd, args) = if super::command_exists("pw-record") {
            (
                "pw-record",
                vec![
                    "--format=s16".to_string(),
                    "--rate=16000".to_string(),
                    "--channels=1".to_string(),
                    self.output_path.to_string_lossy().to_string(),
                ],
            )
        } else if super::command_exists("parecord") {
            (
                "parecord",
                vec![
                    "--format=s16le".to_string(),
                    "--rate=16000".to_string(),
                    "--channels=1".to_string(),
                    "--file-format=wav".to_string(),
                    self.output_path.to_string_lossy().to_string(),
                ],
            )
        } else {
            return Err("No audio recorder found. Install pipewire or pulseaudio".to_string());
        };

        tracing::info!("Starting audio capture with {}", cmd);

        let child = Command::new(cmd)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to start {}: {}", cmd, e))?;

        self.process = Some(child);
        self.is_recording.store(true, Ordering::SeqCst);

        Ok(())
    }

    /// Stop recording and return the audio file path
    pub fn stop(&mut self) -> Result<PathBuf, String> {
        if !self.is_recording() {
            return Err("Not recording".to_string());
        }

        if let Some(mut process) = self.process.take() {
            // Send SIGTERM to allow process to finalize WAV header
            #[cfg(unix)]
            #[allow(unsafe_code)]
            unsafe {
                libc::kill(process.id() as i32, libc::SIGTERM);
            }
            #[cfg(not(unix))]
            let _ = process.kill();
            // Wait for process to finish writing header
            let _ = process.wait();
        }

        self.is_recording.store(false, Ordering::SeqCst);

        // Verify file exists
        if self.output_path.exists() {
            let metadata = fs::metadata(&self.output_path)
                .map_err(|e| format!("Failed to read recording: {}", e))?;

            if metadata.len() < 100 {
                return Err("Recording too short".to_string());
            }

            tracing::info!(
                "Recording saved: {} ({} bytes)",
                self.output_path.display(),
                metadata.len()
            );

            Ok(self.output_path.clone())
        } else {
            Err("Recording file not found".to_string())
        }
    }
}

impl Default for AudioRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AudioRecorder {
    fn drop(&mut self) {
        if self.is_recording() {
            let _ = self.stop();
        }
    }
}
