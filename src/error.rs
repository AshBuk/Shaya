// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Typed errors for ShyQuiet application

use thiserror::Error;

/// UI errors
#[derive(Debug, Error)]
pub enum UiError {
    #[error("Failed to initialize GTK4: {0}")]
    GtkInit(String),

    #[error("Failed to create window: {0}")]
    WindowCreation(String),

    #[error("Layer-shell initialization failed: {0}")]
    LayerShell(String),
}

/// Audio processing errors
#[derive(Debug, Error)]
pub enum AudioError {
    #[error("Failed to encode audio: {0}")]
    EncodingError(String),

    #[error("Invalid sample rate: {0}")]
    InvalidSampleRate(u32),
}
