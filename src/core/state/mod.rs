// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Application state with persistence

use crate::core::models::{
    DEFAULT_ANTHROPIC_MODEL, DEFAULT_GROQ_MODEL, DEFAULT_GROQ_STT_MODEL, DEFAULT_OPENAI_MODEL,
    DEFAULT_OPENAI_STT_MODEL,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

/// AI provider selection
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AiProvider {
    #[default]
    Groq,
    OpenAI,
    Anthropic,
}

impl AiProvider {
    pub const ALL: [AiProvider; 3] = [Self::Groq, Self::OpenAI, Self::Anthropic];

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Groq => "Groq",
            Self::OpenAI => "OpenAI",
            Self::Anthropic => "Anthropic",
        }
    }
}

/// STT provider selection
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SttProvider {
    #[default]
    Groq,
    OpenAI,
}

impl SttProvider {
    pub const ALL: [SttProvider; 2] = [Self::Groq, Self::OpenAI];

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Groq => "Groq",
            Self::OpenAI => "OpenAI",
        }
    }
}

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub provider: AiProvider,
    pub groq_api_key: String,
    pub groq_model: String,
    pub openai_api_key: String,
    pub openai_model: String,
    pub anthropic_api_key: String,
    pub anthropic_model: String,
    pub stt_provider: SttProvider,
    pub groq_stt_api_key: String,
    pub groq_stt_model: String,
    pub openai_stt_api_key: String,
    pub openai_stt_model: String,
    pub dark_mode: bool,
    #[serde(default = "default_opacity")]
    pub window_opacity: f64,
    /// Whether to also record microphone during meetings
    #[serde(default)]
    pub meeting_record_mic: bool,
    #[serde(default = "default_live_chunk_secs")]
    pub live_chunk_secs: u64,
    #[serde(default = "default_live_auto_suggest")]
    pub live_auto_suggest: bool,
    #[serde(default)]
    pub live_debug_transcript: bool,
    #[serde(default = "default_window_x")]
    pub window_x: i32,
    #[serde(default = "default_window_y")]
    pub window_y: i32,
    #[serde(default = "default_window_width")]
    pub window_width: i32,
    #[serde(default = "default_window_height")]
    pub window_height: i32,
}

impl Settings {
    pub fn min_live_chunk_secs(&self) -> u64 {
        match self.stt_provider {
            SttProvider::Groq => 10,
            SttProvider::OpenAI => 5,
        }
    }

    pub fn effective_live_chunk_secs(&self) -> u64 {
        self.live_chunk_secs.clamp(self.min_live_chunk_secs(), 12)
    }

    /// Get API key for the active provider
    pub fn active_api_key(&self) -> &str {
        match self.provider {
            AiProvider::Groq => &self.groq_api_key,
            AiProvider::OpenAI => &self.openai_api_key,
            AiProvider::Anthropic => &self.anthropic_api_key,
        }
    }

    /// Get model for the active provider
    pub fn active_model(&self) -> &str {
        match self.provider {
            AiProvider::Groq => &self.groq_model,
            AiProvider::OpenAI => &self.openai_model,
            AiProvider::Anthropic => &self.anthropic_model,
        }
    }

    /// Set API key for the active provider
    pub fn set_active_api_key(&mut self, key: String) {
        match self.provider {
            AiProvider::Groq => self.groq_api_key = key,
            AiProvider::OpenAI => self.openai_api_key = key,
            AiProvider::Anthropic => self.anthropic_api_key = key,
        }
    }

    /// Set model for the active provider
    pub fn set_active_model(&mut self, model: String) {
        match self.provider {
            AiProvider::Groq => self.groq_model = model,
            AiProvider::OpenAI => self.openai_model = model,
            AiProvider::Anthropic => self.anthropic_model = model,
        }
    }

    /// Get API key for the active STT provider
    pub fn active_stt_key(&self) -> &str {
        match self.stt_provider {
            SttProvider::Groq => &self.groq_stt_api_key,
            SttProvider::OpenAI => &self.openai_stt_api_key,
        }
    }

    /// Get model for the active STT provider
    pub fn active_stt_model(&self) -> &str {
        match self.stt_provider {
            SttProvider::Groq => &self.groq_stt_model,
            SttProvider::OpenAI => &self.openai_stt_model,
        }
    }

    /// Set API key for the active STT provider
    pub fn set_active_stt_key(&mut self, key: String) {
        match self.stt_provider {
            SttProvider::Groq => self.groq_stt_api_key = key,
            SttProvider::OpenAI => self.openai_stt_api_key = key,
        }
    }

    /// Set model for the active STT provider
    pub fn set_active_stt_model(&mut self, model: String) {
        match self.stt_provider {
            SttProvider::Groq => self.groq_stt_model = model,
            SttProvider::OpenAI => self.openai_stt_model = model,
        }
    }
}

fn default_opacity() -> f64 {
    1.0
}
fn default_live_chunk_secs() -> u64 {
    10
}
fn default_live_auto_suggest() -> bool {
    true
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
            provider: AiProvider::default(),
            groq_api_key: String::new(),
            groq_model: DEFAULT_GROQ_MODEL.to_string(),
            openai_api_key: String::new(),
            openai_model: DEFAULT_OPENAI_MODEL.to_string(),
            anthropic_api_key: String::new(),
            anthropic_model: DEFAULT_ANTHROPIC_MODEL.to_string(),
            stt_provider: SttProvider::default(),
            groq_stt_api_key: String::new(),
            groq_stt_model: DEFAULT_GROQ_STT_MODEL.to_string(),
            openai_stt_api_key: String::new(),
            openai_stt_model: DEFAULT_OPENAI_STT_MODEL.to_string(),
            dark_mode: true,
            window_opacity: 1.0,
            meeting_record_mic: false,
            live_chunk_secs: default_live_chunk_secs(),
            live_auto_suggest: default_live_auto_suggest(),
            live_debug_transcript: false,
            window_x: default_window_x(),
            window_y: default_window_y(),
            window_width: default_window_width(),
            window_height: default_window_height(),
        }
    }
}

/// Chat message role
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
}

/// Image data for Vision API (base64-encoded)
#[derive(Debug, Clone)]
pub struct ImageData {
    pub base64_data: String,
    pub mime_type: String,
}

/// Chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    /// Attached image — kept in-memory for current session, not persisted to disk
    #[serde(skip)]
    pub image: Option<ImageData>,
}

/// Conversation history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub messages: Vec<Message>,
    pub created_at: DateTime<Utc>,
}

/// System prompt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemPrompt {
    pub id: String,
    pub name: String,
    pub content: String,
}

/// Cheatsheet note (markdown-rendered)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cheatsheet {
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
    #[serde(default)]
    pub cheatsheets: Vec<Cheatsheet>,
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
            cheatsheets: Vec::new(),
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
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&config_path, &content)?;
        fs::set_permissions(&config_path, fs::Permissions::from_mode(0o600))?;
        tracing::info!("Settings saved to {:?}", config_path);
        Ok(())
    }

    /// Get config file path
    fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("shaya")
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
            self.active_prompt_ids.remove(pos);
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

    /// Remove prompt by ID
    pub fn remove_prompt(&mut self, id: &str) -> bool {
        if let Some(pos) = self.prompts.iter().position(|p| p.id == id) {
            self.prompts.remove(pos);
            self.active_prompt_ids.retain(|i| i != id);
            return true;
        }
        false
    }

    /// Add new cheatsheet, returns its ID
    pub fn add_cheatsheet(&mut self, name: String, content: String) -> String {
        let id = format!("cheat-{}", chrono::Utc::now().timestamp_millis());
        self.cheatsheets.push(Cheatsheet {
            id: id.clone(),
            name,
            content,
        });
        id
    }

    /// Remove cheatsheet by ID
    pub fn remove_cheatsheet(&mut self, id: &str) {
        self.cheatsheets.retain(|c| c.id != id);
    }
}
