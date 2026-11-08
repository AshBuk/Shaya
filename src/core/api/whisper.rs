// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Whisper STT API client (Groq, OpenAI)

use super::{agent, request_error};
use crate::core::state::SttProvider;
use anyhow::Result;
use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const GROQ_WHISPER_URL: &str = "https://api.groq.com/openai/v1/audio/transcriptions";
const OPENAI_WHISPER_URL: &str = "https://api.openai.com/v1/audio/transcriptions";
const MAX_UPLOAD_BYTES: usize = 25 * 1024 * 1024;

/// File name and type to upload under. Our `.opus` files are Ogg containers, and
/// the provider upload lists name `ogg` but not `opus`.
fn upload_identity(path: &Path) -> (String, &'static str) {
    let stem = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("audio");
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("opus" | "ogg") => (format!("{stem}.ogg"), "audio/ogg"),
        _ => (format!("{stem}.wav"), "audio/wav"),
    }
}

/// Whisper STT client
pub struct WhisperClient {
    provider: SttProvider,
    api_key: String,
    model: String,
}

#[derive(Deserialize)]
struct TranscriptionResponse {
    text: String,
}

impl WhisperClient {
    /// Create a new Whisper client
    pub fn new(provider: SttProvider, api_key: &str, model: &str) -> Self {
        Self {
            provider,
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
        let (file_name, content_type) = upload_identity(audio_path);
        if audio_data.len() > MAX_UPLOAD_BYTES {
            anyhow::bail!(
                "{} is {} MB, the upload limit is 25 MB",
                audio_path.display(),
                audio_data.len() / (1024 * 1024)
            );
        }

        tracing::info!("Transcribing {} ({} bytes)", file_name, audio_data.len());

        // Build multipart form (boundary must be unique per RFC 2046)
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let boundary = format!("----ShayaBoundary{}{}", std::process::id(), nonce);
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
        body.extend_from_slice(format!("Content-Type: {}\r\n\r\n", content_type).as_bytes());
        body.extend_from_slice(&audio_data);
        body.extend_from_slice(b"\r\n");

        // Add model field
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"model\"\r\n\r\n");
        body.extend_from_slice(self.model.as_bytes());
        body.extend_from_slice(b"\r\n");

        // Add response format field
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"response_format\"\r\n\r\n");
        body.extend_from_slice(b"json");
        body.extend_from_slice(b"\r\n");

        // End boundary
        body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

        let url = match self.provider {
            SttProvider::Groq => GROQ_WHISPER_URL,
            SttProvider::OpenAI => OPENAI_WHISPER_URL,
        };

        let response = agent()
            .post(url)
            .set("Authorization", &format!("Bearer {}", self.api_key))
            .set(
                "Content-Type",
                &format!("multipart/form-data; boundary={}", boundary),
            )
            .send_bytes(&body)
            .map_err(|e| request_error("Transcription", e))?;

        let transcription: TranscriptionResponse = response.into_json()?;
        let text = transcription.text.trim().to_string();
        // Length only: the text itself is private and logs end up in bug reports.
        tracing::info!("Transcription received ({} chars)", text.chars().count());
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opus_recordings_upload_as_ogg() {
        assert_eq!(
            upload_identity(Path::new("/m/system.opus")),
            ("system.ogg".to_string(), "audio/ogg")
        );
        assert_eq!(
            upload_identity(Path::new("/m/mic_chunk_001.wav")),
            ("mic_chunk_001.wav".to_string(), "audio/wav")
        );
    }
}
