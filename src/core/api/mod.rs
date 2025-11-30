// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! AI API clients

pub mod groq;
pub mod whisper;

pub use groq::GroqClient;
pub use whisper::WhisperClient;
