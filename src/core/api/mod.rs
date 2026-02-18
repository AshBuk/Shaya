// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! AI API clients

pub mod groq;
pub mod whisper;

pub use groq::GroqClient;
pub use whisper::WhisperClient;

use serde::Deserialize;

/// Common API error response (OpenAI-compatible format)
#[derive(Deserialize)]
pub(crate) struct ErrorResponse {
    pub error: ErrorDetail,
}

#[derive(Deserialize)]
pub(crate) struct ErrorDetail {
    pub message: String,
}
