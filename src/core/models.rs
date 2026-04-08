// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! AI model constants

use crate::core::state::{AiProvider, SttProvider};

pub struct ModelInfo {
    pub id: &'static str,
    pub display_name: &'static str,
    pub supports_vision: bool,
}

pub struct SttModelInfo {
    pub id: &'static str,
    pub display_name: &'static str,
}

// --- Groq ---

pub const DEFAULT_GROQ_MODEL: &str = "meta-llama/llama-4-scout-17b-16e-instruct";

/// Vision-capable model for fallback (used when non-Vision model selected + image attached)
pub const VISION_FALLBACK_MODEL: &str = "meta-llama/llama-4-scout-17b-16e-instruct";

pub const GROQ_MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "meta-llama/llama-4-scout-17b-16e-instruct",
        display_name: "Llama 4 Scout 17B",
        supports_vision: true,
    },
    ModelInfo {
        id: "llama-3.3-70b-versatile",
        display_name: "Llama 3.3 70B",
        supports_vision: false,
    },
    ModelInfo {
        id: "llama-3.1-8b-instant",
        display_name: "Llama 3.1 8B",
        supports_vision: false,
    },
    ModelInfo {
        id: "openai/gpt-oss-120b",
        display_name: "GPT-OSS 120B",
        supports_vision: false,
    },
    ModelInfo {
        id: "openai/gpt-oss-20b",
        display_name: "GPT-OSS 20B",
        supports_vision: false,
    },
];

// --- OpenAI ---

pub const DEFAULT_OPENAI_MODEL: &str = "gpt-5.2";

pub const OPENAI_MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "gpt-5.2",
        display_name: "GPT-5.2",
        supports_vision: true,
    },
    ModelInfo {
        id: "gpt-5-mini",
        display_name: "GPT-5 Mini",
        supports_vision: true,
    },
    ModelInfo {
        id: "gpt-5-nano",
        display_name: "GPT-5 Nano",
        supports_vision: true,
    },
    ModelInfo {
        id: "gpt-4.1",
        display_name: "GPT-4.1",
        supports_vision: true,
    },
    ModelInfo {
        id: "gpt-4.1-mini",
        display_name: "GPT-4.1 Mini",
        supports_vision: true,
    },
    ModelInfo {
        id: "o3",
        display_name: "o3 (Reasoning)",
        supports_vision: true,
    },
    ModelInfo {
        id: "o4-mini",
        display_name: "o4 Mini (Reasoning)",
        supports_vision: true,
    },
];

// --- Anthropic ---

pub const DEFAULT_ANTHROPIC_MODEL: &str = "claude-sonnet-4-6";

pub const ANTHROPIC_MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "claude-opus-4-6",
        display_name: "Claude Opus 4.6",
        supports_vision: true,
    },
    ModelInfo {
        id: "claude-sonnet-4-6",
        display_name: "Claude Sonnet 4.6",
        supports_vision: true,
    },
    ModelInfo {
        id: "claude-haiku-4-5",
        display_name: "Claude Haiku 4.5",
        supports_vision: true,
    },
];

// --- STT: Groq ---

pub const DEFAULT_GROQ_STT_MODEL: &str = "whisper-large-v3-turbo";

pub const GROQ_STT_MODELS: &[SttModelInfo] = &[
    SttModelInfo {
        id: "whisper-large-v3-turbo",
        display_name: "Whisper Large V3 Turbo",
    },
    SttModelInfo {
        id: "whisper-large-v3",
        display_name: "Whisper Large V3",
    },
];

// --- STT: OpenAI ---

pub const DEFAULT_OPENAI_STT_MODEL: &str = "gpt-4o-transcribe";

pub const OPENAI_STT_MODELS: &[SttModelInfo] = &[
    SttModelInfo {
        id: "gpt-4o-transcribe",
        display_name: "GPT-4o Transcribe",
    },
    SttModelInfo {
        id: "gpt-4o-mini-transcribe",
        display_name: "GPT-4o Mini Transcribe",
    },
    SttModelInfo {
        id: "whisper-1",
        display_name: "Whisper-1",
    },
];

// --- Helpers ---

/// Get model list for a given provider
pub fn models_for_provider(provider: AiProvider) -> &'static [ModelInfo] {
    match provider {
        AiProvider::Groq => GROQ_MODELS,
        AiProvider::OpenAI => OPENAI_MODELS,
        AiProvider::Anthropic => ANTHROPIC_MODELS,
    }
}

/// Get STT model list for a given provider
pub fn stt_models_for_provider(provider: SttProvider) -> &'static [SttModelInfo] {
    match provider {
        SttProvider::Groq => GROQ_STT_MODELS,
        SttProvider::OpenAI => OPENAI_STT_MODELS,
    }
}

/// Check if model supports Vision (checks across all providers)
pub fn supports_vision(model_id: &str) -> bool {
    [GROQ_MODELS, OPENAI_MODELS, ANTHROPIC_MODELS]
        .iter()
        .flat_map(|list| list.iter())
        .find(|m| m.id == model_id)
        .is_some_and(|m| m.supports_vision)
}
