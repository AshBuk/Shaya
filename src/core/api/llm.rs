// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Unified LLM client with multi-provider support (Groq, OpenAI, Anthropic)

use super::ErrorResponse;
use crate::core::models::{supports_vision, VISION_FALLBACK_MODEL};
use crate::core::state::{AiProvider, Message, Role};
use anyhow::Result;
use serde::Deserialize;
use serde_json::json;

const GROQ_API_URL: &str = "https://api.groq.com/openai/v1/chat/completions";
const OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";
const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

fn max_tokens(provider: AiProvider) -> u32 {
    match provider {
        AiProvider::Groq => 8192,
        AiProvider::OpenAI => 16384,
        AiProvider::Anthropic => 16384,
    }
}

/// Multi-provider LLM client
#[derive(Clone)]
pub struct LlmClient {
    provider: AiProvider,
    api_key: String,
    model: String,
}

// OpenAI-compatible response (Groq & OpenAI)
#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

// Anthropic response
#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(default)]
    text: String,
}

impl LlmClient {
    pub fn new(provider: AiProvider, api_key: &str, model: &str) -> Self {
        Self {
            provider,
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }

    /// Send a chat completion request (images are read from each Message)
    pub fn chat(&self, messages: &[Message], system_prompt: &str) -> Result<String> {
        match self.provider {
            AiProvider::Groq => self.openai_compatible(GROQ_API_URL, messages, system_prompt),
            AiProvider::OpenAI => self.openai_compatible(OPENAI_API_URL, messages, system_prompt),
            AiProvider::Anthropic => self.anthropic(messages, system_prompt),
        }
    }

    /// OpenAI-compatible chat completion (Groq & OpenAI)
    fn openai_compatible(
        &self,
        url: &str,
        messages: &[Message],
        system_prompt: &str,
    ) -> Result<String> {
        let has_images = messages.iter().any(|m| m.image.is_some());
        let model =
            if has_images && self.provider == AiProvider::Groq && !supports_vision(&self.model) {
                tracing::info!(
                    "Image attached but {} doesn't support Vision, falling back to {}",
                    self.model,
                    VISION_FALLBACK_MODEL
                );
                VISION_FALLBACK_MODEL
            } else {
                &self.model
            };

        let mut chat_messages = vec![json!({
            "role": "system",
            "content": system_prompt
        })];

        for msg in messages {
            chat_messages.push(build_openai_message(msg));
        }

        let request = json!({
            "model": model,
            "messages": chat_messages,
            "max_tokens": max_tokens(self.provider)
        });

        let response = ureq::post(url)
            .set("Authorization", &format!("Bearer {}", self.api_key))
            .set("Content-Type", "application/json")
            .send_json(&request);

        match response {
            Ok(resp) => {
                let chat_response: ChatResponse = resp.into_json()?;
                chat_response
                    .choices
                    .first()
                    .map(|c| c.message.content.clone())
                    .ok_or_else(|| anyhow::anyhow!("No response from API"))
            }
            Err(ureq::Error::Status(code, resp)) => {
                let error_text = resp
                    .into_string()
                    .unwrap_or_else(|_| "Unknown error".to_string());
                let provider = self.provider.display_name();
                if let Ok(err) = serde_json::from_str::<ErrorResponse>(&error_text) {
                    anyhow::bail!("{} API error ({}): {}", provider, code, err.error.message)
                } else {
                    anyhow::bail!("{} API error ({}): {}", provider, code, error_text)
                }
            }
            Err(e) => anyhow::bail!("Request failed: {}", e),
        }
    }

    /// Anthropic Messages API
    fn anthropic(&self, messages: &[Message], system_prompt: &str) -> Result<String> {
        let chat_messages: Vec<_> = messages
            .iter()
            .filter(|m| m.role != Role::System)
            .map(build_anthropic_message)
            .collect();

        let request = json!({
            "model": self.model,
            "max_tokens": max_tokens(self.provider),
            "system": system_prompt,
            "messages": chat_messages
        });

        let response = ureq::post(ANTHROPIC_API_URL)
            .set("x-api-key", &self.api_key)
            .set("anthropic-version", ANTHROPIC_VERSION)
            .set("Content-Type", "application/json")
            .send_json(&request);

        match response {
            Ok(resp) => {
                let anthropic_resp: AnthropicResponse = resp.into_json()?;
                anthropic_resp
                    .content
                    .first()
                    .map(|c| c.text.clone())
                    .ok_or_else(|| anyhow::anyhow!("No response from Anthropic"))
            }
            Err(ureq::Error::Status(code, resp)) => {
                let error_text = resp
                    .into_string()
                    .unwrap_or_else(|_| "Unknown error".to_string());
                if let Ok(err) = serde_json::from_str::<ErrorResponse>(&error_text) {
                    anyhow::bail!("Anthropic API error ({}): {}", code, err.error.message)
                } else {
                    anyhow::bail!("Anthropic API error ({}): {}", code, error_text)
                }
            }
            Err(e) => anyhow::bail!("Request failed: {}", e),
        }
    }
}

/// Build OpenAI-format message JSON, embedding image as multimodal content if present
fn build_openai_message(msg: &Message) -> serde_json::Value {
    match &msg.image {
        Some(img) => json!({
            "role": msg.role,
            "content": [
                { "type": "text", "text": msg.content },
                {
                    "type": "image_url",
                    "image_url": {
                        "url": format!("data:{};base64,{}", img.mime_type, img.base64_data)
                    }
                }
            ]
        }),
        None => json!({
            "role": msg.role,
            "content": msg.content
        }),
    }
}

/// Build Anthropic-format message JSON, embedding image as multimodal content if present
fn build_anthropic_message(msg: &Message) -> serde_json::Value {
    match &msg.image {
        Some(img) => json!({
            "role": msg.role,
            "content": [
                {
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": img.mime_type,
                        "data": img.base64_data
                    }
                },
                { "type": "text", "text": msg.content }
            ]
        }),
        None => json!({
            "role": msg.role,
            "content": msg.content
        }),
    }
}
