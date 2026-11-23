// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! AI API clients

pub mod llm;
pub mod whisper;

#[cfg(test)]
mod live_providers;

pub use llm::{list_models, ChatOptions, LlmClient};
pub use whisper::{SpeechSegment, WhisperClient};

use serde::Deserialize;
use std::sync::LazyLock;
use std::time::Duration;

/// Common API error response (OpenAI-compatible format)
#[derive(Deserialize)]
struct ErrorResponse {
    error: ErrorDetail,
}

#[derive(Deserialize)]
struct ErrorDetail {
    message: String,
}

/// Shared HTTP agent. Without a read timeout a stalled connection would keep
/// the send button disabled and a Live request in flight forever.
pub(crate) fn agent() -> &'static ureq::Agent {
    static AGENT: LazyLock<ureq::Agent> = LazyLock::new(|| {
        ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(30))
            .timeout_read(Duration::from_secs(300))
            .timeout_write(Duration::from_secs(300))
            .build()
    });
    &AGENT
}

/// Turn a failed request into an error the user can act on.
pub(crate) fn request_error(api: &str, error: ureq::Error) -> anyhow::Error {
    match error {
        ureq::Error::Status(code, response) => {
            let body = response
                .into_string()
                .unwrap_or_else(|_| "Unknown error".to_string());
            let message = match serde_json::from_str::<ErrorResponse>(&body) {
                Ok(parsed) => parsed.error.message,
                Err(_) => body,
            };
            anyhow::anyhow!("{api} API error ({code}): {message}")
        }
        other => anyhow::anyhow!("Request failed: {other}"),
    }
}
