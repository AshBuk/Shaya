// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Listen recorder - captures system audio (monitor source)
//!
//! Records what the user hears (e.g., meeting audio, video calls).
//! Uses PipeWire/PulseAudio monitor source for loopback capture.

use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Listen recorder - captures system audio output (monitor source)
pub struct ListenRecorder {
    process: Option<Child>,
    output_path: PathBuf,
    is_listening: Arc<AtomicBool>,
}

impl ListenRecorder {
    /// Create a new listen recorder (default temp path)
    pub fn new() -> Self {
        Self::with_path(std::env::temp_dir().join("shyquiet_listen.wav"))
    }

    /// Create a new listen recorder with custom output path
    pub fn with_path(output_path: PathBuf) -> Self {
        Self {
            process: None,
            output_path,
            is_listening: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check if listening is in progress
    pub fn is_listening(&self) -> bool {
        self.is_listening.load(Ordering::SeqCst)
    }

    /// Start listening to system audio
    pub fn start(&mut self) -> Result<(), String> {
        if self.is_listening() {
            return Err("Already listening".to_string());
        }

        // Remove old recording
        let _ = fs::remove_file(&self.output_path);

        // Find monitor source and start recording
        let (cmd, args) = self.build_record_command()?;

        tracing::info!("Starting listen capture with {} {:?}", cmd, args);

        let child = Command::new(cmd)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to start listen capture: {}", e))?;

        self.process = Some(child);
        self.is_listening.store(true, Ordering::SeqCst);

        Ok(())
    }

    /// Stop listening and return the audio file path
    pub fn stop(&mut self) -> Result<PathBuf, String> {
        if !self.is_listening() {
            return Err("Not listening".to_string());
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

        self.is_listening.store(false, Ordering::SeqCst);

        if self.output_path.exists() {
            let metadata = fs::metadata(&self.output_path)
                .map_err(|e| format!("Failed to read listen recording: {}", e))?;

            if metadata.len() < 100 {
                return Err("Listen recording too short".to_string());
            }

            tracing::info!(
                "Listen recording saved: {} ({} bytes)",
                self.output_path.display(),
                metadata.len()
            );

            Ok(self.output_path.clone())
        } else {
            Err("Listen recording file not found".to_string())
        }
    }

    /// Build command for recording monitor source
    fn build_record_command(&self) -> Result<(&'static str, Vec<String>), String> {
        let output = self.output_path.to_string_lossy().to_string();

        // Prefer parecord for monitor sources (better PulseAudio compatibility)
        if crate::core::utils::command_exists("parecord") {
            if let Some(monitor) = Self::find_pulse_monitor() {
                return Ok((
                    "parecord",
                    vec![
                        format!("--device={}", monitor),
                        "--format=s16le".to_string(),
                        "--rate=16000".to_string(),
                        "--channels=1".to_string(),
                        "--file-format=wav".to_string(),
                        output,
                    ],
                ));
            }
        }

        // Fallback to PipeWire native
        if crate::core::utils::command_exists("pw-record") {
            if let Some(monitor) = Self::find_pipewire_monitor() {
                return Ok((
                    "pw-record",
                    vec![
                        "--target".to_string(),
                        monitor,
                        "--format=s16".to_string(),
                        "--rate=16000".to_string(),
                        "--channels=1".to_string(),
                        output,
                    ],
                ));
            }
        }

        Err("No monitor source found. Make sure audio is playing.".to_string())
    }

    /// Find PipeWire monitor source (uses default sink's monitor)
    fn find_pipewire_monitor() -> Option<String> {
        // Use wpctl to get default sink node name
        let output = Command::new("wpctl")
            .args(["inspect", "@DEFAULT_AUDIO_SINK@"])
            .output()
            .ok()?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Find node.name in wpctl output
        for line in stdout.lines() {
            if line.contains("node.name") {
                if let Some(name) = line.split('=').nth(1) {
                    let name = name.trim().trim_matches('"').trim();
                    let monitor = format!("{}.monitor", name);
                    tracing::info!("Found PipeWire default sink monitor: {}", monitor);
                    return Some(monitor);
                }
            }
        }

        // Fallback: try pw-cli
        let output = Command::new("pw-cli").args(["info", "0"]).output().ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        for line in stdout.lines() {
            if line.contains("default.audio.sink") {
                if let Some(name) = line.split('"').nth(1) {
                    let monitor = format!("{}.monitor", name);
                    tracing::info!("Found PipeWire monitor: {}", monitor);
                    return Some(monitor);
                }
            }
        }

        None
    }

    /// Find PulseAudio monitor source (uses default sink's monitor)
    fn find_pulse_monitor() -> Option<String> {
        // Get default sink first
        let output = Command::new("pactl")
            .args(["get-default-sink"])
            .output()
            .ok()?;

        let default_sink = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !default_sink.is_empty() {
            let monitor = format!("{}.monitor", default_sink);
            tracing::info!("Using default sink monitor: {}", monitor);
            return Some(monitor);
        }

        // Fallback: find first monitor source
        let output = Command::new("pactl")
            .args(["list", "short", "sources"])
            .output()
            .ok()?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        for line in stdout.lines() {
            if line.contains(".monitor") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    tracing::info!("Found PulseAudio monitor: {}", parts[1]);
                    return Some(parts[1].to_string());
                }
            }
        }

        None
    }
}

impl Default for ListenRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ListenRecorder {
    fn drop(&mut self) {
        if self.is_listening() {
            let _ = self.stop();
        }
    }
}
