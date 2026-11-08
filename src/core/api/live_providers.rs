// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Real requests to the providers: billed, ignored by default, see CONTRIBUTING.md.

use super::{ChatOptions, LlmClient, WhisperClient};
use crate::core::models::{
    models_for_provider, stt_models_for_provider, DEFAULT_GROQ_STT_MODEL, DEFAULT_OPENAI_STT_MODEL,
};
use crate::core::state::{AiProvider, ChatRoute, ImageData, Message, Role, Settings, SttProvider};
use serde_json::Value;
use std::path::PathBuf;

/// A red 128x128 PNG
const RED_PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAIAAAACACAIAAABMXPacAAAAyElEQVR42u3RQREAAAjDsCmZf1GIQQY8clcFTabVYbEAAAABACAAAAQAgAAAEAAAAgBAAAAIAAABACAAAAQAgAAAEAAAAgBAAAAIAAABACAAAAQAgAAAEAAAAgBAAAAIAAABACAAAAQAgAAAEAAAAgBAAAAAcAEAAAEAIAAABACAAAAQAAACAEAAAAgAAAEAIAAABACAAAAQAAACAEAAAAgAAAEAIAAABACAAAAQAAACAEAAAAgAAAEAIAAABACAAAAQAAAC8KEFIPUEG5PrRbsAAAAASUVORK5CYII=";

const LIMITS: ChatOptions = ChatOptions {
    anthropic_cache: false,
    cache_up_to_message: None,
    anthropic_low_effort: false,
    max_output_tokens: Some(1024),
};

fn key(variable: &str) -> Option<String> {
    let key = std::env::var(variable).ok().filter(|key| !key.is_empty());
    if key.is_none() {
        println!("skipped: {variable} is not set");
    }
    key
}

fn with_provider(provider: AiProvider) -> Settings {
    Settings {
        provider,
        groq_api_key: std::env::var("GROQ_API_KEY").unwrap_or_default(),
        openai_api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
        anthropic_api_key: std::env::var("ANTHROPIC_API_KEY").unwrap_or_default(),
        ..Settings::default()
    }
}

fn message(content: &str, image: Option<ImageData>) -> Message {
    Message {
        role: Role::User,
        content: content.to_string(),
        image,
    }
}

fn ask(route: &ChatRoute, messages: &[Message], options: ChatOptions) -> String {
    let reply = LlmClient::new(route.provider, &route.api_key, &route.model)
        .chat_with_options(messages, "Answer in one word.", options)
        .unwrap_or_else(|e| panic!("{:?} {}: {e}", route.provider, route.model));
    println!("{:?} {}: {reply}", route.provider, route.model);
    reply
}

fn check_text_and_image(settings: &Settings, options: ChatOptions) {
    let text = settings.chat_route(false);
    let reply = ask(&text, &[message("Say the word ready.", None)], options);
    assert!(!reply.trim().is_empty());

    let image = ImageData {
        base64_data: RED_PNG.to_string(),
        mime_type: "image/png".to_string(),
    };
    let route = settings.chat_route(true);
    let question = message("What color is this image?", Some(image));
    let reply = ask(&route, &[question], LIMITS);
    assert!(reply.to_lowercase().contains("red"), "{reply}");
}

#[test]
#[ignore = "billed request"]
fn groq_chat() {
    if key("GROQ_API_KEY").is_some() {
        check_text_and_image(&with_provider(AiProvider::Groq), LIMITS);
    }
}

#[test]
#[ignore = "billed request"]
fn openai_chat() {
    if key("OPENAI_API_KEY").is_some() {
        check_text_and_image(&with_provider(AiProvider::OpenAI), LIMITS);
    }
}

/// Covers the options of Live answers too.
#[test]
#[ignore = "billed request"]
fn anthropic_chat() {
    if key("ANTHROPIC_API_KEY").is_some() {
        let live = ChatOptions {
            anthropic_cache: true,
            cache_up_to_message: Some(0),
            anthropic_low_effort: true,
            ..LIMITS
        };
        check_text_and_image(&with_provider(AiProvider::Anthropic), live);
    }
}

#[test]
#[ignore = "billed request"]
fn image_goes_to_another_provider() {
    if key("GROQ_API_KEY").is_some() && key("ANTHROPIC_API_KEY").is_some() {
        let settings = Settings {
            vision_fallback_provider: AiProvider::Anthropic,
            vision_fallback_model: "claude-haiku-5-5".to_string(),
            ..with_provider(AiProvider::Groq)
        };
        assert!(settings.chat_route(true).image_fallback);
        check_text_and_image(&settings, LIMITS);
    }
}

/// The default image model takes 3 images, and the chat keeps working past that.
#[test]
#[ignore = "billed request"]
fn chat_with_more_images_than_the_model_takes() {
    if key("GROQ_API_KEY").is_some() {
        let image = ImageData {
            base64_data: RED_PNG.to_string(),
            mime_type: "image/png".to_string(),
        };
        let messages: Vec<_> = (0..4)
            .map(|_| message("What color is this image?", Some(image.clone())))
            .collect();

        let route = with_provider(AiProvider::Groq).chat_route(true);
        let reply = ask(&route, &messages, LIMITS);
        assert!(reply.to_lowercase().contains("red"), "{reply}");
    }
}

fn check_transcription(provider: SttProvider, api_key: &str, model: &str) {
    let Some(wav) = std::env::var_os("SHAYA_LIVE_AUDIO").map(PathBuf::from) else {
        println!("skipped: SHAYA_LIVE_AUDIO is not set");
        return;
    };
    let opus = wav.with_extension("opus");
    let client = WhisperClient::new(provider, api_key, model);
    for path in [wav, opus].iter().filter(|path| path.exists()) {
        let text = client
            .transcribe(path)
            .unwrap_or_else(|e| panic!("{provider:?} {model} {}: {e}", path.display()));
        println!("{provider:?} {model} {}: {text}", path.display());
        assert!(text.to_lowercase().contains("test"), "{text}");
    }
}

#[test]
#[ignore = "billed request"]
fn groq_transcription() {
    if let Some(api_key) = key("GROQ_API_KEY") {
        check_transcription(SttProvider::Groq, &api_key, DEFAULT_GROQ_STT_MODEL);
    }
}

#[test]
#[ignore = "billed request"]
fn openai_transcription() {
    if let Some(api_key) = key("OPENAI_API_KEY") {
        check_transcription(SttProvider::OpenAI, &api_key, DEFAULT_OPENAI_STT_MODEL);
    }
}

fn served_model_ids(url: &str, header: (&str, String)) -> Vec<String> {
    let body: Value = ureq::get(url)
        .set(header.0, &header.1)
        .set("anthropic-version", "2023-06-01")
        .call()
        .unwrap_or_else(|e| panic!("{url}: {e}"))
        .into_json()
        .unwrap();
    body["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|model| model["id"].as_str().map(str::to_string))
        .collect()
}

fn missing_from(served: &[String], catalog: Vec<&'static str>) -> Vec<&'static str> {
    catalog
        .into_iter()
        .filter(|id| !served.iter().any(|served| served == id))
        .collect()
}

/// Listing models is free. Reports every provider before failing.
#[test]
#[ignore = "needs API keys"]
fn catalog_ids_are_served() {
    let llm = |provider| models_for_provider(provider).iter().map(|m| m.id);
    let stt = |provider| stt_models_for_provider(provider).iter().map(|m| m.id);
    let mut missing = Vec::new();

    if let Some(api_key) = key("GROQ_API_KEY") {
        let served = served_model_ids(
            "https://api.groq.com/openai/v1/models",
            ("Authorization", format!("Bearer {api_key}")),
        );
        let catalog = llm(AiProvider::Groq).chain(stt(SttProvider::Groq));
        missing.extend(missing_from(&served, catalog.collect()));
    }
    if let Some(api_key) = key("OPENAI_API_KEY") {
        let served = served_model_ids(
            "https://api.openai.com/v1/models",
            ("Authorization", format!("Bearer {api_key}")),
        );
        let catalog = llm(AiProvider::OpenAI).chain(stt(SttProvider::OpenAI));
        missing.extend(missing_from(&served, catalog.collect()));
    }
    if let Some(api_key) = key("ANTHROPIC_API_KEY") {
        let served = served_model_ids(
            "https://api.anthropic.com/v1/models?limit=1000",
            ("x-api-key", api_key),
        );
        missing.extend(missing_from(&served, llm(AiProvider::Anthropic).collect()));
    }

    assert!(missing.is_empty(), "not served: {missing:?}");
}
