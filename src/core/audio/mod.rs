// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Audio processing utilities

pub mod capture;
pub mod chunking;
pub mod encoding;
pub mod listen;
pub mod meeting;
pub mod vad;

pub use capture::AudioRecorder;
pub use listen::ListenRecorder;
pub use vad::{analyze_audio_file, VadConfig};

use std::process::{Command, Stdio};

/// Check if a system command is available on PATH
pub fn command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
