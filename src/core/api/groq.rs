// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Groq API client (blocking) with Vision support

use super::ErrorResponse;
use crate::core::models::{supports_vision, VISION_FALLBACK_MODEL};
use crate::core::state::Message;
use anyhow::Result;
use serde::Deserialize;
use serde_json::json;

const GROQ_API_URL: &str = "https://api.groq.com/openai/v1/chat/completions";

/// Groq API client
#[derive(Clone)]
pub struct GroqClient {
    api_key: String,
    model: String,
}

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

impl GroqClient {
    /// Create a new Groq client
    pub fn new(api_key: &str, model: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }

    /// Send a chat completion request with optional image (Vision API)
    pub fn chat_with_image(
        &self,
        messages: &[Message],
        system_prompt: &str,
        image: Option<&ImageData>,
    ) -> Result<String> {
        // Determine model: fallback to Vision model only if image attached AND current model doesn't support Vision
        let model = if image.is_some() && !supports_vision(&self.model) {
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

        // Add previous messages (without images for history)
        for msg in messages.iter().take(messages.len().saturating_sub(1)) {
            chat_messages.push(json!({
                "role": msg.role,
                "content": msg.content
            }));
        }

        // Add last message with optional image
        if let Some(last_msg) = messages.last() {
            if let Some(img) = image {
                // Vision message with image
                chat_messages.push(json!({
                    "role": last_msg.role,
                    "content": [
                        {
                            "type": "text",
                            "text": last_msg.content
                        },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": format!("data:{};base64,{}", img.mime_type, img.base64_data)
                            }
                        }
                    ]
                }));
            } else {
                chat_messages.push(json!({
                    "role": last_msg.role,
                    "content": last_msg.content
                }));
            }
        }

        let request = json!({
            "model": model,
            "messages": chat_messages,
            "max_tokens": 4096
        });

        let response = ureq::post(GROQ_API_URL)
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
                    .ok_or_else(|| anyhow::anyhow!("No response from Groq"))
            }
            Err(ureq::Error::Status(code, resp)) => {
                let error_text = resp
                    .into_string()
                    .unwrap_or_else(|_| "Unknown error".to_string());

                if let Ok(err) = serde_json::from_str::<ErrorResponse>(&error_text) {
                    anyhow::bail!("Groq API error ({}): {}", code, err.error.message)
                } else {
                    anyhow::bail!("Groq API error ({}): {}", code, error_text)
                }
            }
            Err(e) => anyhow::bail!("Request failed: {}", e),
        }
    }
}

/// Image data for Vision API
#[derive(Clone)]
pub struct ImageData {
    pub base64_data: String,
    pub mime_type: String,
}
