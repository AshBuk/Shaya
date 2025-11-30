// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Audio processing utilities

pub mod capture;
pub mod encoding;
#[allow(dead_code)]
pub mod vad;

pub use capture::AudioRecorder;
