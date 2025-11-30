// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Audio processing utilities

pub mod capture;
pub mod encoding;
pub mod listen;
#[allow(dead_code)]
pub mod vad;

pub use capture::AudioRecorder;
pub use listen::ListenRecorder;
