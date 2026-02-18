// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! AI model constants

pub struct ModelInfo {
    pub id: &'static str,
    pub display_name: &'static str,
    pub supports_vision: bool,
}

pub struct SttModelInfo {
    pub id: &'static str,
    pub display_name: &'static str,
}

/// Vision-capable model for fallback (used when non-Vision model selected + image attached)
pub const VISION_FALLBACK_MODEL: &str = "meta-llama/llama-4-scout-17b-16e-instruct";

pub const MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "meta-llama/llama-4-scout-17b-16e-instruct",
        display_name: "Llama 4 Scout 17B",
        supports_vision: true,
    },
    ModelInfo {
        id: "meta-llama/llama-4-maverick-17b-128e-instruct",
        display_name: "Llama 4 Maverick 17B",
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
];

/// Default model
pub const DEFAULT_MODEL: &str = "meta-llama/llama-4-scout-17b-16e-instruct";

pub const STT_MODELS: &[SttModelInfo] = &[
    SttModelInfo {
        id: "whisper-large-v3-turbo",
        display_name: "Whisper Large V3 Turbo",
    },
    SttModelInfo {
        id: "whisper-large-v3",
        display_name: "Whisper Large V3",
    },
];

/// Default STT model
pub const DEFAULT_STT_MODEL: &str = "whisper-large-v3-turbo";

/// Check if model supports Vision
pub fn supports_vision(model_id: &str) -> bool {
    MODELS
        .iter()
        .find(|m| m.id == model_id)
        .is_some_and(|m| m.supports_vision)
}
