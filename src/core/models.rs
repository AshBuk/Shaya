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

pub const DEFAULT_GROQ_MODEL: &str = "openai/gpt-oss-120b";

/// Default model for requests with images when the selected model cannot read them.
/// Groq has no production vision model: this one is in preview.
pub const DEFAULT_VISION_FALLBACK_MODEL: &str = "qwen/qwen3.8-27b";

/// Groq rejects a request with more images, counting the whole chat history.
const QWEN_MAX_IMAGES: usize = 3;

pub const GROQ_MODELS: &[ModelInfo] = &[
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
    ModelInfo {
        id: "qwen/qwen3.8-27b",
        display_name: "Qwen 3.8 27B (Preview)",
        supports_vision: true,
    },
];

// --- OpenAI ---

pub const DEFAULT_OPENAI_MODEL: &str = "gpt-6.1-sol";

pub const OPENAI_MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "gpt-6-astra",
        display_name: "GPT-6 Astra",
        supports_vision: true,
    },
    ModelInfo {
        id: "gpt-6.1-sol",
        display_name: "GPT-6.1 Sol",
        supports_vision: true,
    },
    ModelInfo {
        id: "gpt-6.1-terra",
        display_name: "GPT-6.1 Terra",
        supports_vision: true,
    },
    ModelInfo {
        id: "gpt-6-luna",
        display_name: "GPT-6 Luna",
        supports_vision: true,
    },
];

// --- Anthropic ---

pub const DEFAULT_ANTHROPIC_MODEL: &str = "claude-sonnet-5-5";

pub const ANTHROPIC_MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "claude-fable-5-1",
        display_name: "Claude Fable 5.1",
        supports_vision: true,
    },
    ModelInfo {
        id: "claude-opus-5-5",
        display_name: "Claude Opus 5.5",
        supports_vision: true,
    },
    ModelInfo {
        id: "claude-sonnet-5-5",
        display_name: "Claude Sonnet 5.5",
        supports_vision: true,
    },
    ModelInfo {
        id: "claude-haiku-5-5",
        display_name: "Claude Haiku 5.5",
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

pub const DEFAULT_OPENAI_STT_MODEL: &str = "gpt-transcribe";

pub const OPENAI_STT_MODELS: &[SttModelInfo] = &[
    SttModelInfo {
        id: "gpt-transcribe",
        display_name: "GPT Transcribe",
    },
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

// --- Migration ---

/// Bump when `LEGACY_MODELS` grows, so saved settings migrate once more.
pub const MODEL_CATALOG_VERSION: u32 = 1;

/// IDs from earlier catalogs. Saved settings move off them once, so a user who enters
/// one of them again as a custom ID keeps it.
pub const LEGACY_MODELS: &[&str] = &[
    "meta-llama/llama-4-scout-17b-16e-instruct",
    "llama-3.3-70b-versatile",
    "llama-3.1-8b-instant",
    "gpt-5.2",
    "gpt-5-mini",
    "gpt-5-nano",
    "gpt-4.1",
    "gpt-4.1-mini",
    "o3",
    "o4-mini",
    "claude-opus-4-6",
    "claude-sonnet-4-6",
    "claude-haiku-4-5",
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

/// Image model offered when a provider is picked for requests with images
pub fn default_vision_model(provider: AiProvider) -> &'static str {
    match provider {
        AiProvider::Groq => DEFAULT_VISION_FALLBACK_MODEL,
        AiProvider::OpenAI => DEFAULT_OPENAI_MODEL,
        AiProvider::Anthropic => DEFAULT_ANTHROPIC_MODEL,
    }
}

/// Get STT model list for a given provider
pub fn stt_models_for_provider(provider: SttProvider) -> &'static [SttModelInfo] {
    match provider {
        SttProvider::Groq => GROQ_STT_MODELS,
        SttProvider::OpenAI => OPENAI_STT_MODELS,
    }
}

fn catalog_entry(model_id: &str) -> Option<&'static ModelInfo> {
    [GROQ_MODELS, OPENAI_MODELS, ANTHROPIC_MODELS]
        .iter()
        .flat_map(|list| list.iter())
        .find(|m| m.id == model_id)
}

/// True only for catalog models known to be text-only. A custom ID is the user's
/// choice, so it receives images as is and the provider reports any problem.
pub fn lacks_vision(model_id: &str) -> bool {
    catalog_entry(model_id).is_some_and(|m| !m.supports_vision)
}

/// Images a model accepts in one request, when a limit is known
pub fn max_images(model_id: &str) -> Option<usize> {
    (model_id == DEFAULT_VISION_FALLBACK_MODEL).then_some(QWEN_MAX_IMAGES)
}

/// Older Claude models answer `output_config.effort` with an error and a custom ID can
/// name one, so only the catalog models receive it. Others run at their default effort.
pub fn supports_effort(model_id: &str) -> bool {
    ANTHROPIC_MODELS.iter().any(|m| m.id == model_id)
}

/// Catalog name of a model, or the ID itself for a custom model
pub fn display_name(model_id: &str) -> &str {
    catalog_entry(model_id).map_or(model_id, |m| m.display_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_in_their_catalogs() {
        let has = |list: &[ModelInfo], id: &str| list.iter().any(|m| m.id == id);
        let has_stt = |list: &[SttModelInfo], id: &str| list.iter().any(|m| m.id == id);

        assert!(has(GROQ_MODELS, DEFAULT_GROQ_MODEL));
        assert!(has(OPENAI_MODELS, DEFAULT_OPENAI_MODEL));
        assert!(has(ANTHROPIC_MODELS, DEFAULT_ANTHROPIC_MODEL));
        assert!(has_stt(GROQ_STT_MODELS, DEFAULT_GROQ_STT_MODEL));
        assert!(has_stt(OPENAI_STT_MODELS, DEFAULT_OPENAI_STT_MODEL));
    }

    #[test]
    fn vision_fallback_can_read_images() {
        assert!(GROQ_MODELS
            .iter()
            .any(|m| m.id == DEFAULT_VISION_FALLBACK_MODEL && m.supports_vision));
    }

    #[test]
    fn default_image_models_read_images() {
        for provider in AiProvider::ALL {
            let id = default_vision_model(provider);
            assert!(models_for_provider(provider)
                .iter()
                .any(|m| m.id == id && m.supports_vision));
        }
    }

    #[test]
    fn only_catalog_text_models_lack_vision() {
        assert!(lacks_vision(DEFAULT_GROQ_MODEL));
        assert!(!lacks_vision(DEFAULT_ANTHROPIC_MODEL));
        assert!(!lacks_vision("my-custom-model"));
    }

    #[test]
    fn legacy_ids_left_the_catalog() {
        assert!(LEGACY_MODELS.iter().all(|id| catalog_entry(id).is_none()));
    }
}
