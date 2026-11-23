// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Unified LLM client: Groq, OpenAI, Anthropic and any OpenAI-compatible server

use super::{agent, request_error};
use crate::core::models::{max_images, supports_effort};
use crate::core::state::{AiProvider, ChatRoute, Message, Role};
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

const ANTHROPIC_VERSION: &str = "2023-06-01";
const ANTHROPIC_MAX_TOKENS: u32 = 16384;
const TRUNCATED_NOTE: &str = "\n\n[Response cut off at the token limit]";

/// Output limit of the OpenAI-format providers. A server the user runs keeps its own
/// default, which the context it was started with bounds.
fn openai_max_tokens(provider: AiProvider) -> Option<u32> {
    match provider {
        AiProvider::Groq => Some(8192),
        AiProvider::OpenAI => Some(16384),
        AiProvider::Anthropic | AiProvider::OpenAICompatible => None,
    }
}

/// Multi-provider LLM client
#[derive(Clone)]
pub struct LlmClient {
    provider: AiProvider,
    api_key: String,
    model: String,
    base_url: String,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ChatOptions {
    pub anthropic_cache: bool,
    pub cache_up_to_message: Option<usize>,
    /// Trade depth for latency on Anthropic models (Live answers).
    pub anthropic_low_effort: bool,
    /// Replace the default output limit
    pub max_output_tokens: Option<u32>,
}

#[derive(Deserialize)]
struct ModelList {
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
}

/// Models an OpenAI-compatible server serves, from its `/models`
pub fn list_models(base_url: &str, api_key: &str) -> Result<Vec<String>> {
    let url = endpoint(base_url, "models");
    let list: ModelList = bearer(agent().get(&url), api_key)
        .call()
        .map_err(|e| match e {
            ureq::Error::Transport(_) => anyhow::anyhow!("Nothing answers at {}", base_url.trim()),
            status => request_error("Server", status),
        })?
        .into_json()
        .map_err(|_| anyhow::anyhow!("{url} returned no model list"))?;
    Ok(list.data.into_iter().map(|m| m.id).collect())
}

fn endpoint(base_url: &str, path: &str) -> String {
    format!("{}/{path}", base_url.trim().trim_end_matches('/'))
}

/// A local server started without a key takes requests without the header.
fn bearer(request: ureq::Request, api_key: &str) -> ureq::Request {
    if api_key.is_empty() {
        request
    } else {
        request.set("Authorization", &format!("Bearer {api_key}"))
    }
}

// OpenAI-compatible response
#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: Option<String>,
    refusal: Option<String>,
}

// Anthropic response
#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
    stop_reason: Option<String>,
    usage: Option<AnthropicUsage>,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    kind: String,
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
            base_url: provider.default_base_url().to_string(),
        }
    }

    pub fn for_route(route: &ChatRoute) -> Self {
        Self {
            base_url: route.base_url.clone(),
            ..Self::new(route.provider, &route.api_key, &route.model)
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
            AiProvider::Anthropic => self.anthropic(messages, system_prompt, options),
            _ => self.openai_compatible(messages, system_prompt, options),
        }
    }

    fn openai_body(
        &self,
        messages: &[Message],
        system_prompt: &str,
        options: ChatOptions,
    ) -> Value {
        let mut chat_messages = Vec::new();
        if !system_prompt.is_empty() {
            chat_messages.push(json!({
                "role": "system",
                "content": system_prompt
            }));
        }

        let first_image = first_sent_image(messages, max_images(&self.model));
        for (idx, msg) in messages.iter().enumerate() {
            chat_messages.push(build_openai_message(msg, idx >= first_image));
        }

        let mut request = json!({
            "model": self.model,
            "messages": chat_messages,
        });
        let limit_field = match self.provider {
            AiProvider::OpenAI => "max_completion_tokens",
            _ => "max_tokens",
        };
        if let Some(limit) = options
            .max_output_tokens
            .or(openai_max_tokens(self.provider))
        {
            request[limit_field] = json!(limit);
        }
        request
    }

    /// OpenAI-format chat completion (all providers but Anthropic)
    fn openai_compatible(
        &self,
        messages: &[Message],
        system_prompt: &str,
        options: ChatOptions,
    ) -> Result<String> {
        let request = agent().post(&endpoint(&self.base_url, "chat/completions"));
        let response = bearer(request, &self.api_key)
            .set("Content-Type", "application/json")
            .send_json(self.openai_body(messages, system_prompt, options))
            .map_err(|e| request_error(self.provider.display_name(), e))?;

        openai_text(response.into_json()?)
    }

    fn anthropic_body(
        &self,
        messages: &[Message],
        system_prompt: &str,
        options: ChatOptions,
    ) -> Value {
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
            "max_tokens": options.max_output_tokens.unwrap_or(ANTHROPIC_MAX_TOKENS),
            "messages": chat_messages
        });
        if !system_prompt.is_empty() {
            request["system"] = anthropic_system(system_prompt, options.anthropic_cache);
        }
        if options.anthropic_low_effort && supports_effort(&self.model) {
            request["output_config"] = json!({ "effort": "low" });
        }
        request
    }

    /// Anthropic Messages API
    fn anthropic(
        &self,
        messages: &[Message],
        system_prompt: &str,
        options: ChatOptions,
    ) -> Result<String> {
        let response = agent()
            .post(&endpoint(&self.base_url, "messages"))
            .set("x-api-key", &self.api_key)
            .set("anthropic-version", ANTHROPIC_VERSION)
            .set("Content-Type", "application/json")
            .send_json(self.anthropic_body(messages, system_prompt, options))
            .map_err(|e| request_error("Anthropic", e))?;

        let anthropic_resp: AnthropicResponse = response.into_json()?;
        if let Some(usage) = &anthropic_resp.usage {
            tracing::info!(
                input_tokens = ?usage.input_tokens,
                output_tokens = ?usage.output_tokens,
                cache_creation_input_tokens = ?usage.cache_creation_input_tokens,
                cache_read_input_tokens = ?usage.cache_read_input_tokens,
                "Anthropic usage"
            );
        }
        anthropic_text(anthropic_resp)
    }
}

fn openai_text(response: ChatResponse) -> Result<String> {
    let choice = response
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No response from API"))?;
    if let Some(refusal) = choice.message.refusal.filter(|r| !r.is_empty()) {
        anyhow::bail!("The model declined the request: {}", refusal);
    }
    let truncated = choice.finish_reason.as_deref() == Some("length");
    finish_text(choice.message.content.unwrap_or_default(), truncated)
}

/// Replies can hold thinking and other blocks around the text, in any position.
fn anthropic_text(response: AnthropicResponse) -> Result<String> {
    let stop_reason = response.stop_reason.as_deref();
    if stop_reason == Some("refusal") {
        anyhow::bail!("The model declined the request");
    }
    let text = response
        .content
        .iter()
        .filter(|block| block.kind == "text")
        .map(|block| block.text.as_str())
        .collect::<String>();
    finish_text(text, stop_reason == Some("max_tokens"))
}

fn finish_text(text: String, truncated: bool) -> Result<String> {
    match (text.trim().is_empty(), truncated) {
        (true, true) => anyhow::bail!("The token limit was reached before any answer was written"),
        (true, false) => anyhow::bail!("The API returned an empty response"),
        (false, true) => Ok(text + TRUNCATED_NOTE),
        (false, false) => Ok(text),
    }
}

/// Index of the first message whose image goes into the request. Images before it stay
/// out, so a model with an image limit keeps answering about the newest ones and a long
/// chat does not fail. Their text and the replies about them are still sent.
fn first_sent_image(messages: &[Message], limit: Option<usize>) -> usize {
    let Some(limit) = limit else {
        return 0;
    };
    messages
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, msg)| msg.image.is_some())
        .nth(limit)
        .map_or(0, |(idx, _)| idx + 1)
}

/// Build OpenAI-format message JSON, embedding image as multimodal content if present
fn build_openai_message(msg: &Message, with_image: bool) -> serde_json::Value {
    match msg.image.as_ref().filter(|_| with_image) {
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

    fn anthropic_reply(body: Value) -> Result<String> {
        anthropic_text(serde_json::from_value(body).unwrap())
    }

    fn openai_reply(body: Value) -> Result<String> {
        openai_text(serde_json::from_value(body).unwrap())
    }

    #[test]
    fn anthropic_text_skips_other_blocks() {
        let reply = anthropic_reply(json!({
            "stop_reason": "end_turn",
            "content": [
                { "type": "thinking", "thinking": "", "signature": "s" },
                { "type": "text", "text": "Hello " },
                { "type": "text", "text": "there" }
            ]
        }));

        assert_eq!(reply.unwrap(), "Hello there");
    }

    #[test]
    fn anthropic_refusal_is_an_error() {
        let reply = anthropic_reply(json!({ "stop_reason": "refusal", "content": [] }));

        assert!(reply.unwrap_err().to_string().contains("declined"));
    }

    #[test]
    fn anthropic_thinking_only_reply_is_an_error_not_a_blank_message() {
        let reply = anthropic_reply(json!({
            "stop_reason": "max_tokens",
            "content": [{ "type": "thinking", "thinking": "", "signature": "s" }]
        }));

        assert!(reply.is_err());
    }

    #[test]
    fn truncated_reply_keeps_the_text_and_says_so() {
        let reply = anthropic_reply(json!({
            "stop_reason": "max_tokens",
            "content": [{ "type": "text", "text": "partial" }]
        }));

        assert_eq!(reply.unwrap(), format!("partial{TRUNCATED_NOTE}"));
    }

    #[test]
    fn openai_null_content_is_an_error_not_a_parse_failure() {
        let reply = openai_reply(json!({
            "choices": [{ "message": { "content": null }, "finish_reason": "stop" }]
        }));

        assert!(reply.unwrap_err().to_string().contains("empty"));
    }

    #[test]
    fn openai_refusal_is_reported() {
        let reply = openai_reply(json!({
            "choices": [{
                "message": { "content": null, "refusal": "not this" },
                "finish_reason": "stop"
            }]
        }));

        assert!(reply.unwrap_err().to_string().contains("not this"));
    }

    #[test]
    fn output_limit_field_depends_on_provider() {
        let messages = [user_message("hi")];
        let openai = LlmClient::new(AiProvider::OpenAI, "k", "gpt-6.1-sol");
        let groq = LlmClient::new(AiProvider::Groq, "k", "openai/gpt-oss-120b");

        let openai_body = openai.openai_body(&messages, "", ChatOptions::default());
        let groq_body = groq.openai_body(&messages, "", ChatOptions::default());

        assert!(openai_body.get("max_tokens").is_none());
        assert_eq!(openai_body["max_completion_tokens"], 16384);
        assert_eq!(groq_body["max_tokens"], 8192);
    }

    #[test]
    fn compatible_server_keeps_its_own_output_limit() {
        let messages = [user_message("hi")];
        let local = LlmClient::new(AiProvider::OpenAICompatible, "", "qwen3.5-4b");
        let lowered = ChatOptions {
            max_output_tokens: Some(100),
            ..ChatOptions::default()
        };

        assert!(local
            .openai_body(&messages, "", ChatOptions::default())
            .get("max_tokens")
            .is_none());
        assert_eq!(local.openai_body(&messages, "", lowered)["max_tokens"], 100);
    }

    #[test]
    fn endpoint_tolerates_a_trailing_slash() {
        assert_eq!(
            endpoint(" http://127.0.0.1:8080/v1/ ", "models"),
            "http://127.0.0.1:8080/v1/models"
        );
        assert_eq!(
            endpoint(AiProvider::Groq.default_base_url(), "chat/completions"),
            "https://api.groq.com/openai/v1/chat/completions"
        );
    }

    #[test]
    fn output_limit_can_be_lowered() {
        let options = ChatOptions {
            max_output_tokens: Some(100),
            ..ChatOptions::default()
        };
        let messages = [user_message("hi")];
        let openai = LlmClient::new(AiProvider::OpenAI, "k", "gpt-6.1-sol");
        let anthropic = LlmClient::new(AiProvider::Anthropic, "k", "claude-sonnet-5-5");

        assert_eq!(
            openai.openai_body(&messages, "", options)["max_completion_tokens"],
            100
        );
        assert_eq!(
            anthropic.anthropic_body(&messages, "", options)["max_tokens"],
            100
        );
    }

    #[test]
    fn anthropic_options_reach_the_request() {
        let messages = [user_message("hi")];
        let options = ChatOptions {
            anthropic_low_effort: true,
            ..ChatOptions::default()
        };
        let sonnet = LlmClient::new(AiProvider::Anthropic, "k", "claude-sonnet-5-5");
        let haiku = LlmClient::new(AiProvider::Anthropic, "k", "claude-haiku-5-5");

        let sonnet_body = sonnet.anthropic_body(&messages, "", options);
        let haiku_body = haiku.anthropic_body(&messages, "", ChatOptions::default());

        assert_eq!(sonnet_body["output_config"]["effort"], "low");
        assert!(sonnet_body.get("fallbacks").is_none());
        assert!(haiku_body.get("output_config").is_none());
    }

    #[test]
    fn effort_is_not_sent_to_a_custom_model() {
        let options = ChatOptions {
            anthropic_low_effort: true,
            ..ChatOptions::default()
        };
        let custom = LlmClient::new(AiProvider::Anthropic, "k", "claude-haiku-4-5-20251001");

        let body = custom.anthropic_body(&[user_message("hi")], "", options);

        assert!(body.get("output_config").is_none());
    }

    #[test]
    fn only_the_newest_images_fit_the_model_limit() {
        let mut messages: Vec<_> = (0..5)
            .map(|i| {
                let mut msg = user_message(&i.to_string());
                msg.image = Some(ImageData {
                    base64_data: "abc123".to_string(),
                    mime_type: "image/png".to_string(),
                });
                msg
            })
            .collect();
        messages.push(user_message("no image"));
        let image_sent = |model: &str| {
            let client = LlmClient::new(AiProvider::Groq, "k", model);
            let body = client.openai_body(&messages, "", ChatOptions::default());
            body["messages"]
                .as_array()
                .unwrap()
                .iter()
                .map(|msg| msg["content"].is_array())
                .collect::<Vec<_>>()
        };

        assert_eq!(
            image_sent("qwen/qwen3.8-27b"),
            [false, false, true, true, true, false]
        );
        assert_eq!(
            image_sent("my-custom-model"),
            [true, true, true, true, true, false]
        );
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
