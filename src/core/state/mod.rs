// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Application state with persistence

use crate::core::models::{
    display_name, lacks_vision, DEFAULT_ANTHROPIC_MODEL, DEFAULT_GROQ_MODEL,
    DEFAULT_GROQ_STT_MODEL, DEFAULT_OPENAI_MODEL, DEFAULT_OPENAI_STT_MODEL,
    DEFAULT_VISION_FALLBACK_MODEL, LEGACY_MODELS, MODEL_CATALOG_VERSION,
};
use crate::core::utils;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const LIVE_CHUNK_MIN_SECS: u64 = 5;
pub const LIVE_CHUNK_MAX_SECS: u64 = 12;

/// AI provider selection
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AiProvider {
    #[default]
    Groq,
    OpenAI,
    Anthropic,
    /// A server the user runs, such as llama.cpp, Ollama, LM Studio or vLLM
    #[serde(rename = "openai-compatible")]
    OpenAICompatible,
}

impl AiProvider {
    pub const ALL: [AiProvider; 4] = [
        Self::Groq,
        Self::OpenAI,
        Self::Anthropic,
        Self::OpenAICompatible,
    ];

    /// Providers with a model catalog, offered for requests with images
    pub const CLOUD: [AiProvider; 3] = [Self::Groq, Self::OpenAI, Self::Anthropic];

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Groq => "Groq",
            Self::OpenAI => "OpenAI",
            Self::Anthropic => "Anthropic",
            Self::OpenAICompatible => "OpenAI-compatible",
        }
    }

    /// API root, up to and including the version segment
    pub fn default_base_url(self) -> &'static str {
        match self {
            Self::Groq => "https://api.groq.com/openai/v1",
            Self::OpenAI => "https://api.openai.com/v1",
            Self::Anthropic => "https://api.anthropic.com/v1",
            Self::OpenAICompatible => "http://127.0.0.1:8080/v1",
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

/// Application settings. Fields missing from an older file take their default value.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub provider: AiProvider,
    pub groq_api_key: String,
    pub groq_model: String,
    pub openai_api_key: String,
    pub openai_model: String,
    pub anthropic_api_key: String,
    pub anthropic_model: String,
    /// Server of the OpenAI-compatible provider, up to and including `/v1`
    pub compatible_base_url: String,
    /// Optional: llama.cpp without `--api-key` and Ollama take none
    pub compatible_api_key: String,
    pub compatible_model: String,
    /// The server has no catalog, so the user tells whether its model reads images.
    pub compatible_vision: bool,
    /// Serves requests with images when the selected model cannot read them
    pub vision_fallback_provider: AiProvider,
    pub vision_fallback_model: String,
    /// Catalog version these settings were migrated to. A file without it predates migration.
    #[serde(default)]
    pub model_catalog: u32,
    pub stt_provider: SttProvider,
    pub groq_stt_api_key: String,
    pub groq_stt_model: String,
    pub openai_stt_api_key: String,
    pub openai_stt_model: String,
    pub dark_mode: bool,
    pub window_opacity: f64,
    /// Whether to also record microphone during meetings
    pub meeting_record_mic: bool,
    pub live_chunk_secs: u64,
    pub live_auto_suggest: bool,
    pub live_debug_transcript: bool,
    pub window_x: i32,
    pub window_y: i32,
    pub window_width: i32,
    pub window_height: i32,
}

/// Where a chat request goes
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatRoute {
    pub provider: AiProvider,
    pub api_key: String,
    pub model: String,
    pub base_url: String,
    /// The selected model cannot read images, so this request uses the image model.
    pub image_fallback: bool,
}

impl ChatRoute {
    /// What the user still has to enter in Settings before a request can go out
    pub fn missing(&self) -> Option<&'static str> {
        match self.provider {
            AiProvider::OpenAICompatible if self.base_url.trim().is_empty() => {
                Some("Set the server URL in Settings")
            }
            AiProvider::OpenAICompatible => None,
            _ if self.api_key.is_empty() => Some("Set an API key in Settings"),
            _ => None,
        }
    }

    /// Tells the user that images went to another model, which can be another provider.
    pub fn image_notice(&self) -> Option<String> {
        self.image_fallback.then(|| {
            format!(
                "images go to {} {}",
                self.provider.display_name(),
                display_name(&self.model)
            )
        })
    }
}

impl Settings {
    /// Clean up saved model IDs: blank ones, and once, IDs of earlier catalogs.
    /// Without this, requests keep using an ID the provider no longer serves.
    pub fn normalize_models(&mut self) {
        let migrate = self.model_catalog < MODEL_CATALOG_VERSION;
        let refresh = |model: &mut String, default: &str| {
            if model.trim().is_empty() || (migrate && LEGACY_MODELS.contains(&model.as_str())) {
                tracing::info!("Model {:?} is no longer offered, using {}", model, default);
                *model = default.to_string();
            }
        };

        refresh(&mut self.groq_model, DEFAULT_GROQ_MODEL);
        refresh(&mut self.openai_model, DEFAULT_OPENAI_MODEL);
        refresh(&mut self.anthropic_model, DEFAULT_ANTHROPIC_MODEL);
        refresh(&mut self.groq_stt_model, DEFAULT_GROQ_STT_MODEL);
        refresh(&mut self.openai_stt_model, DEFAULT_OPENAI_STT_MODEL);
        if self.vision_fallback_model.trim().is_empty() {
            self.vision_fallback_provider = AiProvider::Groq;
            self.vision_fallback_model = DEFAULT_VISION_FALLBACK_MODEL.to_string();
        }
        self.model_catalog = MODEL_CATALOG_VERSION;
    }

    fn api_key_for(&self, provider: AiProvider) -> &str {
        match provider {
            AiProvider::Groq => &self.groq_api_key,
            AiProvider::OpenAI => &self.openai_api_key,
            AiProvider::Anthropic => &self.anthropic_api_key,
            AiProvider::OpenAICompatible => &self.compatible_api_key,
        }
    }

    fn base_url_for(&self, provider: AiProvider) -> &str {
        match provider {
            AiProvider::OpenAICompatible => &self.compatible_base_url,
            cloud => cloud.default_base_url(),
        }
    }

    fn active_lacks_vision(&self) -> bool {
        match self.provider {
            AiProvider::OpenAICompatible => !self.compatible_vision,
            _ => lacks_vision(self.active_model()),
        }
    }

    /// Pick the provider and model for a request
    pub fn chat_route(&self, has_images: bool) -> ChatRoute {
        let image_fallback = has_images && self.active_lacks_vision();
        let (provider, model) = if image_fallback {
            (
                self.vision_fallback_provider,
                self.vision_fallback_model.as_str(),
            )
        } else {
            (self.provider, self.active_model())
        };
        ChatRoute {
            provider,
            api_key: self.api_key_for(provider).to_string(),
            model: model.to_string(),
            base_url: self.base_url_for(provider).to_string(),
            image_fallback,
        }
    }

    pub fn effective_live_chunk_secs(&self) -> u64 {
        self.live_chunk_secs
            .clamp(LIVE_CHUNK_MIN_SECS, LIVE_CHUNK_MAX_SECS)
    }

    /// Get API key for the active provider
    pub fn active_api_key(&self) -> &str {
        self.api_key_for(self.provider)
    }

    /// Get model for the active provider
    pub fn active_model(&self) -> &str {
        match self.provider {
            AiProvider::Groq => &self.groq_model,
            AiProvider::OpenAI => &self.openai_model,
            AiProvider::Anthropic => &self.anthropic_model,
            AiProvider::OpenAICompatible => &self.compatible_model,
        }
    }

    /// Set API key for the active provider
    pub fn set_active_api_key(&mut self, key: String) {
        match self.provider {
            AiProvider::Groq => self.groq_api_key = key,
            AiProvider::OpenAI => self.openai_api_key = key,
            AiProvider::Anthropic => self.anthropic_api_key = key,
            AiProvider::OpenAICompatible => self.compatible_api_key = key,
        }
    }

    /// Set model for the active provider
    pub fn set_active_model(&mut self, model: String) {
        match self.provider {
            AiProvider::Groq => self.groq_model = model,
            AiProvider::OpenAI => self.openai_model = model,
            AiProvider::Anthropic => self.anthropic_model = model,
            AiProvider::OpenAICompatible => self.compatible_model = model,
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
            compatible_base_url: AiProvider::OpenAICompatible.default_base_url().to_string(),
            compatible_api_key: String::new(),
            compatible_model: String::new(),
            compatible_vision: true,
            vision_fallback_provider: AiProvider::Groq,
            vision_fallback_model: DEFAULT_VISION_FALLBACK_MODEL.to_string(),
            model_catalog: MODEL_CATALOG_VERSION,
            stt_provider: SttProvider::default(),
            groq_stt_api_key: String::new(),
            groq_stt_model: DEFAULT_GROQ_STT_MODEL.to_string(),
            openai_stt_api_key: String::new(),
            openai_stt_model: DEFAULT_OPENAI_STT_MODEL.to_string(),
            dark_mode: true,
            window_opacity: 1.0,
            meeting_record_mic: false,
            live_chunk_secs: 10,
            live_auto_suggest: true,
            live_debug_transcript: false,
            window_x: 100,
            window_y: 100,
            window_width: 400,
            window_height: 550,
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[serde(default)]
pub struct AppState {
    pub settings: Settings,
    pub history: Vec<Conversation>,
    pub prompts: Vec<SystemPrompt>,
    /// Active prompt IDs (multiple prompts combined for AI)
    pub active_prompt_ids: Vec<String>,
    pub cheatsheets: Vec<Cheatsheet>,
    #[serde(skip)]
    pub current_messages: Vec<Message>,
    /// Changes whenever the open chat is replaced, so a request that finishes
    /// later can tell its result belongs to a chat that is gone.
    #[serde(skip)]
    pub chat_generation: u64,
    /// Shown to the user when the saved state could not be loaded.
    #[serde(skip)]
    pub load_error: Option<String>,
    /// Set when an unreadable state file could not be moved aside: saving would destroy it.
    #[serde(skip)]
    save_blocked: bool,
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
            chat_generation: 0,
            load_error: None,
            save_blocked: false,
        }
    }
}

impl AppState {
    /// Load state from disk or create default
    pub fn load() -> Self {
        Self::load_from(&Self::config_path())
    }

    fn load_from(path: &Path) -> Self {
        let parsed = match fs::read_to_string(path) {
            Ok(content) => serde_json::from_str::<Self>(&content).map_err(|e| e.to_string()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Self::default(),
            Err(e) => Err(e.to_string()),
        };
        match parsed {
            Ok(mut state) => {
                state.settings.normalize_models();
                state
            }
            Err(reason) => Self::recover_from_unreadable(path, &reason),
        }
    }

    /// Start with defaults without losing the old file, which holds API keys and history.
    fn recover_from_unreadable(path: &Path, reason: &str) -> Self {
        tracing::error!("Failed to load {:?}: {}", path, reason);
        let backup = path.with_extension(format!("corrupt-{}.json", Utc::now().timestamp()));
        let (message, save_blocked) = match fs::rename(path, &backup) {
            Ok(()) => (
                format!(
                    "Settings could not be read ({reason}). The old file was kept at {}",
                    backup.display()
                ),
                false,
            ),
            Err(e) => (
                format!(
                    "Settings could not be read ({reason}) or moved aside ({e}). \
                     Saving is off so {} stays intact",
                    path.display()
                ),
                true,
            ),
        };
        Self {
            load_error: Some(message),
            save_blocked,
            ..Self::default()
        }
    }

    /// Save state to disk
    pub fn save(&self) -> anyhow::Result<()> {
        self.save_to(&Self::config_path())
    }

    fn save_to(&self, path: &Path) -> anyhow::Result<()> {
        if self.save_blocked {
            anyhow::bail!("Saving is off because {} could not be read", path.display());
        }
        if let Some(parent) = path.parent() {
            utils::ensure_private_dir(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        utils::write_private(path, content.as_bytes())?;
        tracing::info!("Settings saved to {:?}", path);
        Ok(())
    }

    /// Replace the open chat. Requests started for the previous one drop their result.
    pub fn replace_chat(&mut self, messages: Vec<Message>) {
        self.current_messages = messages;
        self.chat_generation += 1;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn config_in(dir: &tempfile::TempDir) -> PathBuf {
        dir.path().join("settings.json")
    }

    #[test]
    fn saved_state_round_trips_with_private_mode() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = config_in(&dir);
        let mut state = AppState::default();
        state.settings.groq_api_key = "secret".to_string();

        state.save_to(&path).unwrap();

        assert_eq!(
            AppState::load_from(&path).settings.groq_api_key,
            "secret".to_string()
        );
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn corrupt_file_is_kept_and_reported() {
        let dir = tempfile::tempdir().unwrap();
        let path = config_in(&dir);
        fs::write(&path, r#"{"settings": {"groq_api_key": "sec"#).unwrap();

        let state = AppState::load_from(&path);

        assert!(state.load_error.is_some());
        assert!(state.history.is_empty());
        assert!(!path.exists());
        let kept: Vec<_> = fs::read_dir(dir.path()).unwrap().collect();
        assert_eq!(kept.len(), 1);
        let content = fs::read_to_string(kept[0].as_ref().unwrap().path()).unwrap();
        assert!(content.contains("groq_api_key"));

        // Saving defaults afterwards leaves the kept copy alone.
        state.save_to(&path).unwrap();
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn save_is_refused_when_unreadable_file_could_not_be_moved() {
        let dir = tempfile::tempdir().unwrap();
        let path = config_in(&dir);
        let state = AppState {
            save_blocked: true,
            ..AppState::default()
        };
        fs::write(&path, "original").unwrap();

        assert!(state.save_to(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "original");
    }

    #[test]
    fn missing_fields_take_defaults_instead_of_resetting_everything() {
        let dir = tempfile::tempdir().unwrap();
        let path = config_in(&dir);
        fs::write(&path, r#"{"settings": {"groq_api_key": "kept"}}"#).unwrap();

        let state = AppState::load_from(&path);

        assert!(state.load_error.is_none());
        assert_eq!(state.settings.groq_api_key, "kept");
        assert_eq!(state.settings.window_width, 400);
        assert_eq!(state.prompts.len(), 1);
    }

    #[test]
    fn retired_model_ids_move_to_the_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = config_in(&dir);
        fs::write(
            &path,
            r#"{"settings": {
                "groq_model": "meta-llama/llama-4-scout-17b-16e-instruct",
                "anthropic_model": "claude-haiku-5-5"
            }}"#,
        )
        .unwrap();

        let settings = AppState::load_from(&path).settings;

        assert_eq!(settings.groq_model, DEFAULT_GROQ_MODEL);
        assert_eq!(settings.anthropic_model, "claude-haiku-5-5");
    }

    #[test]
    fn custom_model_ids_survive_loading() {
        let dir = tempfile::tempdir().unwrap();
        let path = config_in(&dir);
        let mut state = AppState::default();
        state.settings.openai_model = "my-custom-model".to_string();
        state.settings.anthropic_model = "claude-opus-4-6".to_string();
        state.save_to(&path).unwrap();

        let settings = AppState::load_from(&path).settings;

        assert_eq!(settings.openai_model, "my-custom-model");
        assert_eq!(settings.anthropic_model, "claude-opus-4-6");
    }

    #[test]
    fn text_model_sends_images_to_the_configured_provider() {
        let settings = Settings {
            groq_api_key: "groq".to_string(),
            anthropic_api_key: "anthropic".to_string(),
            vision_fallback_provider: AiProvider::Anthropic,
            vision_fallback_model: "claude-haiku-5-5".to_string(),
            ..Settings::default()
        };

        let text = settings.chat_route(false);
        assert_eq!(text.provider, AiProvider::Groq);
        assert_eq!(text.model, DEFAULT_GROQ_MODEL);
        assert_eq!(text.image_notice(), None);

        let image = settings.chat_route(true);
        assert_eq!(image.provider, AiProvider::Anthropic);
        assert_eq!(image.api_key, "anthropic");
        assert_eq!(image.model, "claude-haiku-5-5");
        assert!(image.image_notice().unwrap().contains("Anthropic"));
    }

    #[test]
    fn vision_and_custom_models_keep_their_images() {
        let vision = Settings {
            provider: AiProvider::Anthropic,
            ..Settings::default()
        };
        let custom = Settings {
            groq_model: "my-vision-model".to_string(),
            ..Settings::default()
        };

        assert!(!vision.chat_route(true).image_fallback);
        assert_eq!(custom.chat_route(true).model, "my-vision-model");
    }

    #[test]
    fn compatible_server_needs_a_url_not_a_key() {
        let mut settings = Settings {
            provider: AiProvider::OpenAICompatible,
            ..Settings::default()
        };

        let route = settings.chat_route(false);
        assert_eq!(route.missing(), None);
        assert_eq!(route.base_url, "http://127.0.0.1:8080/v1");

        settings.compatible_base_url = " ".to_string();
        assert!(settings.chat_route(false).missing().is_some());
        assert!(Settings::default().chat_route(false).missing().is_some());
    }

    #[test]
    fn compatible_text_model_sends_images_to_the_image_model() {
        let mut settings = Settings {
            provider: AiProvider::OpenAICompatible,
            compatible_model: "qwen3.5-4b".to_string(),
            ..Settings::default()
        };
        assert_eq!(settings.chat_route(true).model, "qwen3.5-4b");

        settings.compatible_vision = false;
        let route = settings.chat_route(true);
        assert_eq!(route.provider, AiProvider::Groq);
        assert_eq!(route.base_url, AiProvider::Groq.default_base_url());
    }

    #[test]
    fn replacing_the_chat_changes_its_generation() {
        let mut state = AppState::default();
        let before = state.chat_generation;

        state.replace_chat(Vec::new());

        assert_ne!(state.chat_generation, before);
    }
}
