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
