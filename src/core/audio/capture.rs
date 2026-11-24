// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Audio capture using system tools (pw-record/parecord)

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::PathBuf;
use std::process::Child;

/// Audio recorder using PipeWire or PulseAudio
pub struct AudioRecorder {
    process: Option<Child>,
    output_path: PathBuf,
    is_recording: bool,
}

impl AudioRecorder {
    /// Create an idle audio recorder
    pub fn new() -> Self {
        Self {
            process: None,
            output_path: PathBuf::new(),
            is_recording: false,
        }
    }

    /// Check if recording is in progress
    pub fn is_recording(&self) -> bool {
        self.is_recording
    }

    /// Start recording audio into `output_path`
    pub fn start(&mut self, output_path: PathBuf) -> Result<()> {
        if self.is_recording() {
            bail!("Already recording");
        }

        self.output_path = output_path;
        let _ = fs::remove_file(&self.output_path);

        // Try pw-record first (PipeWire), fallback to parecord (PulseAudio)
        let (cmd, args) = if crate::core::utils::command_exists("pw-record") {
            (
                "pw-record",
                vec![
                    "--format=s16".to_string(),
                    "--rate=16000".to_string(),
                    "--channels=1".to_string(),
                    self.output_path.to_string_lossy().to_string(),
                ],
            )
        } else if crate::core::utils::command_exists("parecord") {
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
            bail!("No audio recorder found. Install pipewire or pulseaudio");
        };

        tracing::info!("Starting audio capture with {}", cmd);

        let child = crate::core::utils::spawn_recorder(cmd, &args)
            .with_context(|| format!("Failed to start {cmd}"))?;

        self.process = Some(child);
        self.is_recording = true;

        Ok(())
    }

    /// Stop recording and return the audio file path
    pub fn stop(&mut self) -> Result<PathBuf> {
        if !self.is_recording() {
            bail!("Not recording");
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

        self.is_recording = false;

        // Verify file exists
        if self.output_path.exists() {
            let metadata = fs::metadata(&self.output_path).context("Failed to read recording")?;

            if metadata.len() < 100 {
                let _ = fs::remove_file(&self.output_path);
                bail!("Recording too short");
            }

            tracing::info!(
                "Recording saved: {} ({} bytes)",
                self.output_path.display(),
                metadata.len()
            );

            Ok(self.output_path.clone())
        } else {
            bail!("Recording file not found")
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
