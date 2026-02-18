// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Whisper STT API client (Groq)

use super::ErrorResponse;
use anyhow::Result;
use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const GROQ_WHISPER_URL: &str = "https://api.groq.com/openai/v1/audio/transcriptions";

/// Whisper STT client
pub struct WhisperClient {
    api_key: String,
    model: String,
}

#[derive(Deserialize)]
struct TranscriptionResponse {
    text: String,
}

impl WhisperClient {
    /// Create a new Whisper client
    pub fn new(api_key: &str, model: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }

    /// Transcribe audio file to text
    pub fn transcribe(&self, audio_path: &Path) -> Result<String> {
        if !audio_path.exists() {
            anyhow::bail!("Audio file not found: {}", audio_path.display());
        }

        let audio_data = fs::read(audio_path)?;
        let file_name = audio_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("audio.wav");

        tracing::info!("Transcribing {} ({} bytes)", file_name, audio_data.len());

        // Build multipart form (boundary must be unique per RFC 2046)
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let boundary = format!("----ShyQuietBoundary{}{}", std::process::id(), nonce);
        let mut body = Vec::new();

        // Add file field
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(
            format!(
                "Content-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\n",
                file_name
            )
            .as_bytes(),
        );
        body.extend_from_slice(b"Content-Type: audio/wav\r\n\r\n");
        body.extend_from_slice(&audio_data);
        body.extend_from_slice(b"\r\n");

        // Add model field
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"model\"\r\n\r\n");
        body.extend_from_slice(self.model.as_bytes());
        body.extend_from_slice(b"\r\n");

        // Add language field (optional, auto-detect)
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"response_format\"\r\n\r\n");
        body.extend_from_slice(b"json");
        body.extend_from_slice(b"\r\n");

        // End boundary
        body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

        let response = ureq::post(GROQ_WHISPER_URL)
            .set("Authorization", &format!("Bearer {}", self.api_key))
            .set(
                "Content-Type",
                &format!("multipart/form-data; boundary={}", boundary),
            )
            .send_bytes(&body);

        match response {
            Ok(resp) => {
                let transcription: TranscriptionResponse = resp.into_json()?;
                let text = transcription.text.trim().to_string();
                tracing::info!("Transcription: {}", text);
                Ok(text)
            }
            Err(ureq::Error::Status(code, resp)) => {
                let error_text = resp.into_string().unwrap_or_else(|_| "Unknown".to_string());

                if let Ok(err) = serde_json::from_str::<ErrorResponse>(&error_text) {
                    anyhow::bail!("Whisper API error ({}): {}", code, err.error.message)
                } else {
                    anyhow::bail!("Whisper API error ({}): {}", code, error_text)
                }
            }
            Err(e) => anyhow::bail!("Request failed: {}", e),
        }
    }
}
