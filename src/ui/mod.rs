// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! UI Implementation
//!
//! Unified stealth window with ViewStack:
//! - Chat View - AI interaction
//! - Settings View - API keys, models
//! - History View - Past conversations
//! - Prompts View - System prompts editor

mod header;
mod layer_shell;
pub mod stealth;
pub mod views;
