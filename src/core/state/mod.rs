// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Application state with persistence

use crate::core::models::{DEFAULT_MODEL, DEFAULT_STT_MODEL};
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
    #[serde(default = "default_opacity")]
    pub window_opacity: f64,
    /// Whether to also record microphone during meetings
    #[serde(default)]
    pub meeting_record_mic: bool,
    #[serde(default = "default_window_x")]
    pub window_x: i32,
    #[serde(default = "default_window_y")]
    pub window_y: i32,
    #[serde(default = "default_window_width")]
    pub window_width: i32,
    #[serde(default = "default_window_height")]
    pub window_height: i32,
}

fn default_opacity() -> f64 {
    1.0
}
fn default_window_x() -> i32 {
    100
}
fn default_window_y() -> i32 {
    100
}
fn default_window_width() -> i32 {
    400
}
fn default_window_height() -> i32 {
    550
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: DEFAULT_MODEL.to_string(),
            stt_api_key: String::new(),
            stt_model: DEFAULT_STT_MODEL.to_string(),
            dark_mode: true,
            window_opacity: 1.0,
            meeting_record_mic: false,
            window_x: default_window_x(),
            window_y: default_window_y(),
            window_width: default_window_width(),
            window_height: default_window_height(),
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
    /// Active prompt IDs (multiple prompts combined for AI)
    pub active_prompt_ids: Vec<String>,
    #[serde(skip)]
    pub current_messages: Vec<Message>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            history: Vec::new(),
            prompts: vec![SystemPrompt::default()],
            active_prompt_ids: vec!["default".to_string()],
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

    /// Get combined active system prompts content
    pub fn active_prompt(&self) -> String {
        self.prompts
            .iter()
            .filter(|p| self.active_prompt_ids.contains(&p.id))
            .map(|p| p.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Check if prompt is active
    pub fn is_prompt_active(&self, id: &str) -> bool {
        self.active_prompt_ids.iter().any(|i| i == id)
    }

    /// Toggle prompt active state
    pub fn toggle_prompt(&mut self, id: &str) {
        if let Some(pos) = self.active_prompt_ids.iter().position(|i| i == id) {
            // Don't allow deactivating the last prompt
            if self.active_prompt_ids.len() > 1 {
                self.active_prompt_ids.remove(pos);
            }
        } else {
            self.active_prompt_ids.push(id.to_string());
        }
    }

    /// Add new prompt, returns its ID
    pub fn add_prompt(&mut self, name: String, content: String) -> String {
        let id = format!("prompt-{}", chrono::Utc::now().timestamp_millis());
        self.prompts.push(SystemPrompt {
            id: id.clone(),
            name,
            content,
        });
        id
    }

    /// Remove prompt by ID (can't remove last one)
    pub fn remove_prompt(&mut self, id: &str) -> bool {
        if self.prompts.len() <= 1 {
            return false;
        }
        if let Some(pos) = self.prompts.iter().position(|p| p.id == id) {
            self.prompts.remove(pos);
            self.active_prompt_ids.retain(|i| i != id);
            return true;
        }
        false
    }
}
