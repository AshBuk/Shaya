// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Application state with persistence

use crate::core::models::DEFAULT_MODEL;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub api_key: String,
    pub model: String,
    pub stt_api_key: String,
    pub stt_model: String,
    pub dark_mode: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: DEFAULT_MODEL.to_string(),
            stt_api_key: String::new(),
            stt_model: "whisper-large-v3-turbo".to_string(),
            dark_mode: true,
        }
    }
}

/// Chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// Conversation history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub messages: Vec<Message>,
    pub created_at: String,
}

/// System prompt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemPrompt {
    pub id: String,
    pub name: String,
    pub content: String,
}

impl Default for SystemPrompt {
    fn default() -> Self {
        Self {
            id: "default".to_string(),
            name: "Default".to_string(),
            content: "You are a helpful AI assistant.".to_string(),
        }
    }
}

/// Main application state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppState {
    pub settings: Settings,
    pub history: Vec<Conversation>,
    pub prompts: Vec<SystemPrompt>,
    #[serde(skip)]
    pub current_messages: Vec<Message>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            history: Vec::new(),
            prompts: vec![SystemPrompt::default()],
            current_messages: Vec::new(),
        }
    }
}

impl AppState {
    /// Load state from disk or create default
    pub fn load() -> Self {
        let config_path = Self::config_path();
        if config_path.exists() {
            match fs::read_to_string(&config_path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(state) => return state,
                    Err(e) => tracing::warn!("Failed to parse config: {}", e),
                },
                Err(e) => tracing::warn!("Failed to read config: {}", e),
            }
        }
        Self::default()
    }

    /// Save state to disk
    pub fn save(&self) -> anyhow::Result<()> {
        let config_path = Self::config_path();
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&config_path, content)?;
        tracing::info!("Settings saved to {:?}", config_path);
        Ok(())
    }

    /// Get config file path
    fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("shyquiet")
            .join("settings.json")
    }

    /// Check if API key is configured
    #[allow(dead_code)]
    pub fn has_api_key(&self) -> bool {
        !self.settings.api_key.is_empty()
    }
}
