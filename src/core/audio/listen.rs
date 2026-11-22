// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Listen recorder - captures system audio (monitor source)
//!
//! Records what the user hears (e.g., meeting audio, video calls).
//! Uses PipeWire/PulseAudio monitor source for loopback capture.

use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Listen recorder - captures system audio output (monitor source)
pub struct ListenRecorder {
    process: Option<Child>,
    output_path: PathBuf,
    is_listening: Arc<AtomicBool>,
}

impl ListenRecorder {
    /// Create an idle listen recorder
    pub fn new() -> Self {
        Self {
            process: None,
            output_path: PathBuf::new(),
            is_listening: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check if listening is in progress
    pub fn is_listening(&self) -> bool {
        self.is_listening.load(Ordering::SeqCst)
    }

    /// True when the capture process died while it should still be recording.
    pub fn has_exited(&mut self) -> bool {
        self.process
            .as_mut()
            .is_some_and(|process| matches!(process.try_wait(), Ok(Some(_))))
    }

    /// Start recording system audio into `output_path`
    pub fn start(&mut self, output_path: PathBuf) -> Result<(), String> {
        if self.is_listening() {
            return Err("Already listening".to_string());
        }

        self.output_path = output_path;
        let _ = fs::remove_file(&self.output_path);

        // Find monitor source and start recording
        let (cmd, args) = self.build_record_command()?;

        tracing::info!("Starting listen capture with {} {:?}", cmd, args);

        let child = crate::core::utils::spawn_recorder(cmd, &args)
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
            // Signal only a child that was not reaped yet: has_exited() may have reaped
            // it, and its PID can already belong to another process.
            if matches!(process.try_wait(), Ok(None)) {
                // Send SIGTERM to allow process to finalize WAV header
                #[cfg(unix)]
                #[allow(unsafe_code)]
                unsafe {
                    libc::kill(process.id() as i32, libc::SIGTERM);
                }
                #[cfg(not(unix))]
                let _ = process.kill();
            }
            // Wait for process to finish writing header
            let _ = process.wait();
        }

        self.is_listening.store(false, Ordering::SeqCst);

        if self.output_path.exists() {
            let metadata = fs::metadata(&self.output_path)
                .map_err(|e| format!("Failed to read listen recording: {}", e))?;

            if metadata.len() < 100 {
                let _ = fs::remove_file(&self.output_path);
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
            if let Some(sink) = Self::find_pipewire_sink() {
                return Ok((
                    "pw-record",
                    vec![
                        // Record what the sink plays. PipeWire has no `<sink>.monitor`
                        // node, and an unknown target falls back to the microphone.
                        "-P".to_string(),
                        "{ stream.capture.sink=true }".to_string(),
                        "--target".to_string(),
                        sink,
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

    /// Find the default PipeWire sink, recorded through its monitor ports
    fn find_pipewire_sink() -> Option<String> {
        let sink = command_stdout("wpctl", &["inspect", "@DEFAULT_AUDIO_SINK@"])
            .and_then(|stdout| parse_wpctl_sink(&stdout))
            .or_else(|| {
                command_stdout("pw-metadata", &["0", "default.audio.sink"])
                    .and_then(|stdout| parse_pw_metadata_sink(&stdout))
            })?;

        tracing::info!("Found PipeWire default sink: {}", sink);
        Some(sink)
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

fn command_stdout(cmd: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(cmd).args(args).output().ok()?;
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Read the sink name from `  * node.name = "name"` in `wpctl inspect` output.
fn parse_wpctl_sink(stdout: &str) -> Option<String> {
    let value = stdout
        .lines()
        .filter(|line| line.contains("node.name"))
        .find_map(|line| line.split('=').nth(1))?;
    Some(value.trim().trim_matches('"').trim().to_string())
}

/// Read the sink name from the JSON value in `pw-metadata` output.
fn parse_pw_metadata_sink(stdout: &str) -> Option<String> {
    let line = stdout
        .lines()
        .find(|line| line.contains("key:'default.audio.sink'"))?;
    let (_, rest) = line.split_once("value:'")?;
    let (value, _) = rest.split_once("' type:")?;
    let json: serde_json::Value = serde_json::from_str(value).ok()?;
    json["name"].as_str().map(str::to_owned)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::utils::spawn_recorder;
    use std::time::{Duration, Instant};

    fn running(cmd: &str, args: &[String], output_path: PathBuf) -> ListenRecorder {
        ListenRecorder {
            process: Some(spawn_recorder(cmd, args).unwrap()),
            output_path,
            is_listening: Arc::new(AtomicBool::new(true)),
        }
    }

    #[test]
    fn wpctl_output_gives_the_sink_name() {
        let stdout = "id 58, type PipeWire:Interface:Node\n  * node.description = \"Speakers\"\n  * node.name = \"alsa_output.analog-stereo\"\n";
        assert_eq!(
            parse_wpctl_sink(stdout).as_deref(),
            Some("alsa_output.analog-stereo")
        );
        assert_eq!(parse_wpctl_sink(""), None);
    }

    #[test]
    fn pw_metadata_output_gives_the_sink_name() {
        let stdout = "Found \"default\" metadata 40\nupdate: id:0 key:'default.audio.sink' value:'{\"name\":\"alsa_output.analog-stereo\"}' type:'Spa:String:JSON'\n";
        assert_eq!(
            parse_pw_metadata_sink(stdout).as_deref(),
            Some("alsa_output.analog-stereo")
        );
        assert_eq!(
            parse_pw_metadata_sink("Found \"default\" metadata 40\n"),
            None
        );
    }

    #[test]
    fn stop_after_detected_exit_still_returns_the_recording() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("system.wav");
        fs::write(&path, [0u8; 200]).unwrap();
        let mut recorder = running("true", &[], path.clone());

        let deadline = Instant::now() + Duration::from_secs(5);
        while !recorder.has_exited() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }

        assert_eq!(recorder.stop().unwrap(), path);
    }

    #[test]
    fn stop_terminates_a_running_capture() {
        let dir = tempfile::tempdir().unwrap();
        let args = ["30".to_string()];
        let mut recorder = running("sleep", &args, dir.path().join("system.wav"));
        assert!(!recorder.has_exited());

        let started = Instant::now();
        let _ = recorder.stop();

        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
