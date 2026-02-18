// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! AI model constants

/// Vision-capable model for fallback (used when non-Vision model selected + image attached)
pub const VISION_FALLBACK_MODEL: &str = "meta-llama/llama-4-scout-17b-16e-instruct";

/// Available models for selection: (model_id, display_name, supports_vision)
pub const MODELS: &[(&str, &str, bool)] = &[
    (
        "meta-llama/llama-4-scout-17b-16e-instruct",
        "Llama 4 Scout 17B",
        true,
    ),
    (
        "meta-llama/llama-4-maverick-17b-128e-instruct",
        "Llama 4 Maverick 17B",
        true,
    ),
    ("llama-3.3-70b-versatile", "Llama 3.3 70B", false),
    ("llama-3.1-8b-instant", "Llama 3.1 8B", false),
];

/// Default model
pub const DEFAULT_MODEL: &str = "meta-llama/llama-4-scout-17b-16e-instruct";

/// Available STT models: (model_id, display_name)
pub const STT_MODELS: &[(&str, &str)] = &[
    ("whisper-large-v3-turbo", "Whisper Large V3 Turbo"),
    ("whisper-large-v3", "Whisper Large V3"),
];

/// Default STT model
pub const DEFAULT_STT_MODEL: &str = "whisper-large-v3-turbo";

/// Check if model supports Vision
pub fn supports_vision(model_id: &str) -> bool {
    MODELS
        .iter()
        .find(|(id, _, _)| *id == model_id)
        .map(|(_, _, vision)| *vision)
        .unwrap_or(false)
}
