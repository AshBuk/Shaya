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
use serde_json::{json, Value};

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

#[derive(Debug, Default, Clone, Copy)]
pub struct ChatOptions {
    pub anthropic_cache: bool,
    pub cache_up_to_message: Option<usize>,
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
    usage: Option<AnthropicUsage>,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(default)]
    text: String,
}

#[derive(Deserialize)]
struct AnthropicUsage {
    input_tokens: Option<u32>,
    output_tokens: Option<u32>,
    cache_creation_input_tokens: Option<u32>,
    cache_read_input_tokens: Option<u32>,
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
        self.chat_with_options(messages, system_prompt, ChatOptions::default())
    }

    /// Send a chat completion request with provider-specific request options.
    pub fn chat_with_options(
        &self,
        messages: &[Message],
        system_prompt: &str,
        options: ChatOptions,
    ) -> Result<String> {
        match self.provider {
            AiProvider::Groq => self.openai_compatible(GROQ_API_URL, messages, system_prompt),
            AiProvider::OpenAI => self.openai_compatible(OPENAI_API_URL, messages, system_prompt),
            AiProvider::Anthropic => self.anthropic(messages, system_prompt, options),
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

        let mut chat_messages = Vec::new();
        if !system_prompt.is_empty() {
            chat_messages.push(json!({
                "role": "system",
                "content": system_prompt
            }));
        }

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
    fn anthropic(
        &self,
        messages: &[Message],
        system_prompt: &str,
        options: ChatOptions,
    ) -> Result<String> {
        let chat_messages: Vec<_> = messages
            .iter()
            .filter(|msg| msg.role != Role::System)
            .enumerate()
            .map(|(idx, msg)| {
                build_anthropic_message_with_cache(
                    msg,
                    options.anthropic_cache && options.cache_up_to_message == Some(idx),
                )
            })
            .collect();

        let mut request = json!({
            "model": self.model,
            "max_tokens": max_tokens(self.provider),
            "messages": chat_messages
        });
        if !system_prompt.is_empty() {
            request["system"] = anthropic_system(system_prompt, options.anthropic_cache);
        }

        let response = ureq::post(ANTHROPIC_API_URL)
            .set("x-api-key", &self.api_key)
            .set("anthropic-version", ANTHROPIC_VERSION)
            .set("Content-Type", "application/json")
            .send_json(&request);

        match response {
            Ok(resp) => {
                let anthropic_resp: AnthropicResponse = resp.into_json()?;
                if let Some(usage) = &anthropic_resp.usage {
                    tracing::info!(
                        input_tokens = ?usage.input_tokens,
                        output_tokens = ?usage.output_tokens,
                        cache_creation_input_tokens = ?usage.cache_creation_input_tokens,
                        cache_read_input_tokens = ?usage.cache_read_input_tokens,
                        "Anthropic usage"
                    );
                }
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

fn cache_control() -> Value {
    json!({ "type": "ephemeral" })
}

fn anthropic_system(system_prompt: &str, cache: bool) -> Value {
    if cache {
        json!([
            {
                "type": "text",
                "text": system_prompt,
                "cache_control": cache_control()
            }
        ])
    } else {
        json!(system_prompt)
    }
}

fn anthropic_text_block(text: &str, cache: bool) -> Value {
    if cache {
        json!({
            "type": "text",
            "text": text,
            "cache_control": cache_control()
        })
    } else {
        json!({
            "type": "text",
            "text": text
        })
    }
}

fn anthropic_image_block(img: &crate::core::state::ImageData, cache: bool) -> Value {
    if cache {
        json!({
            "type": "image",
            "source": {
                "type": "base64",
                "media_type": img.mime_type,
                "data": img.base64_data
            },
            "cache_control": cache_control()
        })
    } else {
        json!({
            "type": "image",
            "source": {
                "type": "base64",
                "media_type": img.mime_type,
                "data": img.base64_data
            }
        })
    }
}

/// Build Anthropic-format message JSON, embedding image as multimodal content if present
fn build_anthropic_message_with_cache(msg: &Message, cache: bool) -> serde_json::Value {
    match &msg.image {
        Some(img) => {
            let text_is_empty = msg.content.trim().is_empty();
            let content = if text_is_empty {
                vec![anthropic_image_block(img, cache)]
            } else {
                vec![
                    anthropic_image_block(img, false),
                    anthropic_text_block(&msg.content, cache),
                ]
            };
            json!({
                "role": msg.role,
                "content": content
            })
        }
        None if cache => json!({
            "role": msg.role,
            "content": [anthropic_text_block(&msg.content, true)]
        }),
        None => json!({
            "role": msg.role,
            "content": msg.content
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::state::ImageData;

    fn user_message(content: &str) -> Message {
        Message {
            role: Role::User,
            content: content.to_string(),
            image: None,
        }
    }

    #[test]
    fn anthropic_system_cache_uses_content_blocks() {
        let system = anthropic_system("system prompt", true);

        assert_eq!(system[0]["type"], "text");
        assert_eq!(system[0]["text"], "system prompt");
        assert_eq!(system[0]["cache_control"]["type"], "ephemeral");
    }

    #[test]
    fn default_anthropic_system_stays_plain_string() {
        assert_eq!(
            anthropic_system("system prompt", false),
            json!("system prompt")
        );
    }

    #[test]
    fn cached_text_message_marks_text_block() {
        let payload = build_anthropic_message_with_cache(&user_message("hello"), true);

        assert_eq!(payload["content"][0]["type"], "text");
        assert_eq!(payload["content"][0]["text"], "hello");
        assert_eq!(payload["content"][0]["cache_control"]["type"], "ephemeral");
    }

    #[test]
    fn uncached_text_message_stays_plain_string() {
        let payload = build_anthropic_message_with_cache(&user_message("hello"), false);

        assert_eq!(payload["content"], json!("hello"));
    }

    #[test]
    fn cached_image_message_marks_final_text_block() {
        let mut msg = user_message("explain this");
        msg.image = Some(ImageData {
            base64_data: "abc123".to_string(),
            mime_type: "image/png".to_string(),
        });

        let payload = build_anthropic_message_with_cache(&msg, true);

        assert_eq!(payload["content"][0]["type"], "image");
        assert!(payload["content"][0].get("cache_control").is_none());
        assert_eq!(payload["content"][1]["type"], "text");
        assert_eq!(payload["content"][1]["cache_control"]["type"], "ephemeral");
    }

    #[test]
    fn cached_image_only_message_omits_empty_text_block() {
        let mut msg = user_message("");
        msg.image = Some(ImageData {
            base64_data: "abc123".to_string(),
            mime_type: "image/png".to_string(),
        });

        let payload = build_anthropic_message_with_cache(&msg, true);

        assert_eq!(payload["content"].as_array().unwrap().len(), 1);
        assert_eq!(payload["content"][0]["type"], "image");
        assert_eq!(payload["content"][0]["cache_control"]["type"], "ephemeral");
    }
}
