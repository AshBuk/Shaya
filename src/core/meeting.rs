// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Meeting data model and directory-based persistence
//!
//! Each meeting is stored in its own directory under
//! `~/.local/share/shyquiet/meetings/meeting-{timestamp}/`

use crate::core::api::WhisperClient;
use crate::core::audio::chunking;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Meeting recording status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MeetingStatus {
    Recording,
    Converting,
    Completed,
    Transcribing,
    Transcribed,
    Analyzed,
}

impl std::fmt::Display for MeetingStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Recording => write!(f, "Recording"),
            Self::Converting => write!(f, "Converting"),
            Self::Completed => write!(f, "Completed"),
            Self::Transcribing => write!(f, "Transcribing"),
            Self::Transcribed => write!(f, "Transcribed"),
            Self::Analyzed => write!(f, "Analyzed"),
        }
    }
}

/// Meeting metadata (persisted as metadata.json in meeting directory)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meeting {
    pub id: String,
    pub title: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub duration_secs: u64,
    pub status: MeetingStatus,
    pub has_mic: bool,
    pub audio_format: String,
}

impl Meeting {
    /// Create a new meeting with auto-generated ID
    pub fn new(has_mic: bool) -> Self {
        let now = chrono::Utc::now();
        let id = format!("meeting-{}", now.timestamp_millis());
        let title = format!("Meeting {}", now.format("%Y-%m-%d %H:%M"));
        Self {
            id,
            title,
            started_at: now,
            duration_secs: 0,
            status: MeetingStatus::Recording,
            has_mic,
            audio_format: "wav".to_string(),
        }
    }

    /// Format started_at for display
    pub fn started_at_display(&self) -> String {
        self.started_at.format("%Y-%m-%d %H:%M").to_string()
    }
}

/// Directory-based meeting persistence
pub struct MeetingManager;

impl MeetingManager {
    /// Base directory for all meetings
    pub fn base_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("shyquiet")
            .join("meetings")
    }

    /// Get directory for a specific meeting
    pub fn meeting_dir(meeting_id: &str) -> PathBuf {
        Self::base_dir().join(meeting_id)
    }

    /// Save meeting metadata to its directory
    pub fn save(meeting: &Meeting) -> Result<()> {
        let dir = Self::meeting_dir(&meeting.id);
        fs::create_dir_all(&dir)?;
        let path = dir.join("metadata.json");
        let content = serde_json::to_string_pretty(meeting)?;
        fs::write(&path, content)?;
        tracing::info!("Meeting saved: {}", meeting.id);
        Ok(())
    }

    /// Load meeting metadata from its directory
    pub fn load(meeting_id: &str) -> Result<Meeting> {
        let path = Self::meeting_dir(meeting_id).join("metadata.json");
        let content = fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&content)?)
    }

    /// List all meetings sorted by date (newest first)
    pub fn list() -> Vec<Meeting> {
        let base = Self::base_dir();
        if !base.exists() {
            return Vec::new();
        }

        let mut meetings: Vec<Meeting> = fs::read_dir(&base)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                let entry = entry.ok()?;
                if !entry.path().is_dir() {
                    return None;
                }
                let id = entry.file_name().to_string_lossy().to_string();
                Self::load(&id).ok()
            })
            .collect();

        // Newest first (by ID which contains timestamp)
        meetings.sort_by(|a, b| b.id.cmp(&a.id));
        meetings
    }

    /// Delete a meeting and its directory
    pub fn delete(meeting_id: &str) -> Result<()> {
        let dir = Self::meeting_dir(meeting_id);
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
            tracing::info!("Meeting deleted: {}", meeting_id);
        }
        Ok(())
    }

    /// Get transcript text for a meeting (if available)
    pub fn get_transcript(meeting_id: &str) -> Option<String> {
        let path = Self::meeting_dir(meeting_id).join("transcript.txt");
        fs::read_to_string(&path).ok()
    }

    /// Save transcript text for a meeting
    pub fn save_transcript(meeting_id: &str, transcript: &str) -> Result<()> {
        let path = Self::meeting_dir(meeting_id).join("transcript.txt");
        fs::write(&path, transcript)?;
        Ok(())
    }

    /// Get analysis text for a meeting (if available)
    pub fn get_analysis(meeting_id: &str) -> Option<String> {
        let path = Self::meeting_dir(meeting_id).join("analysis.md");
        fs::read_to_string(&path).ok()
    }

    /// Save analysis text for a meeting
    pub fn save_analysis(meeting_id: &str, analysis: &str) -> Result<()> {
        let path = Self::meeting_dir(meeting_id).join("analysis.md");
        fs::write(&path, analysis)?;
        Ok(())
    }

    /// Find WAV files (before OPUS conversion)
    pub fn find_wav_files(meeting_id: &str) -> Vec<PathBuf> {
        let dir = Self::meeting_dir(meeting_id);
        ["system.wav", "mic.wav"]
            .iter()
            .map(|name| dir.join(name))
            .filter(|p| p.exists())
            .collect()
    }

    /// Find OPUS files (after conversion)
    pub fn find_opus_files(meeting_id: &str) -> Vec<PathBuf> {
        let dir = Self::meeting_dir(meeting_id);
        ["system.opus", "mic.opus"]
            .iter()
            .map(|name| dir.join(name))
            .filter(|p| p.exists())
            .collect()
    }

    /// Find any audio file for playback (prefers OPUS, falls back to WAV)
    pub fn find_playback_file(meeting_id: &str) -> Option<PathBuf> {
        let dir = Self::meeting_dir(meeting_id);
        ["system.opus", "system.wav", "mic.opus", "mic.wav"]
            .iter()
            .map(|name| dir.join(name))
            .find(|p| p.exists())
    }

    /// Find audio files for transcription: WAV first (chunkable), OPUS as fallback
    pub fn find_transcribable_files(meeting_id: &str) -> Vec<PathBuf> {
        let wav = Self::find_wav_files(meeting_id);
        if !wav.is_empty() {
            return wav;
        }
        Self::find_opus_files(meeting_id)
    }

    /// Format duration as human-readable string
    pub fn format_duration(secs: u64) -> String {
        let hours = secs / 3600;
        let mins = (secs % 3600) / 60;
        let secs = secs % 60;
        if hours > 0 {
            format!("{}h {:02}m {:02}s", hours, mins, secs)
        } else {
            format!("{}m {:02}s", mins, secs)
        }
    }
}

/// Transcribe a meeting's audio files.
/// WAV files are chunked via hound; OPUS files are sent directly to Whisper API.
/// Call this from a blocking thread (not the GTK main loop).
pub fn transcribe_meeting(meeting_id: &str, stt_api_key: &str, stt_model: &str) -> Result<String> {
    let audio_files = MeetingManager::find_transcribable_files(meeting_id);
    if audio_files.is_empty() {
        anyhow::bail!("No audio files found for meeting {}", meeting_id);
    }

    let is_wav = audio_files
        .first()
        .and_then(|p| p.extension())
        .is_some_and(|ext| ext == "wav");

    let client = WhisperClient::new(stt_api_key, stt_model);
    let mut full_transcript = String::new();
    let multi_source = audio_files.len() > 1;

    if is_wav {
        // WAV: chunk with hound (handles files > 25MB)
        let chunks_dir = chunks_dir(meeting_id);
        let mut all_chunks: Vec<(PathBuf, &str)> = Vec::new();
        for path in &audio_files {
            let label = source_label(path);
            for chunk in chunking::chunk_wav(path, &chunks_dir)? {
                all_chunks.push((chunk, label));
            }
        }

        let total = all_chunks.len();
        tracing::info!("Transcribing {} WAV chunks for {}", total, meeting_id);

        for (idx, (chunk_path, label)) in all_chunks.iter().enumerate() {
            tracing::info!("Chunk {}/{}", idx + 1, total);
            append_transcription(
                &client,
                chunk_path,
                label,
                multi_source,
                &mut full_transcript,
            );
        }

        cleanup_chunks(meeting_id)?;
    } else {
        // OPUS: send directly (Whisper accepts ogg/opus, ~14MB/hour at 32kbps)
        tracing::info!(
            "Transcribing {} OPUS files for {}",
            audio_files.len(),
            meeting_id
        );
        for path in &audio_files {
            let label = source_label(path);
            append_transcription(&client, path, label, multi_source, &mut full_transcript);
        }
    }

    // Save transcript and update status
    MeetingManager::save_transcript(meeting_id, &full_transcript)?;
    if let Ok(mut meeting) = MeetingManager::load(meeting_id) {
        meeting.status = MeetingStatus::Transcribed;
        MeetingManager::save(&meeting)?;
    }

    tracing::info!(
        "Transcription complete for {} ({} chars)",
        meeting_id,
        full_transcript.len()
    );
    Ok(full_transcript)
}

/// Determine source label from audio file name
fn source_label(path: &Path) -> &'static str {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if name.starts_with("mic") {
        "[Mic]"
    } else {
        "[System]"
    }
}

/// Transcribe a single file and append result to transcript
fn append_transcription(
    client: &WhisperClient,
    path: &Path,
    label: &str,
    multi_source: bool,
    transcript: &mut String,
) {
    match client.transcribe(path) {
        Ok(text) if !text.is_empty() => {
            if !transcript.is_empty() {
                transcript.push('\n');
            }
            if multi_source {
                transcript.push_str(&format!("{} {}", label, text));
            } else {
                transcript.push_str(&text);
            }
        }
        Ok(_) => {} // empty text, skip
        Err(e) => {
            tracing::warn!("Transcription error for {}: {}", path.display(), e);
            transcript.push_str(&format!("\n[Transcription error: {}]\n", e));
        }
    }
}

/// Analyze a meeting transcript using AI.
/// Call this from a blocking thread.
pub fn analyze_meeting(meeting_id: &str, api_key: &str, model: &str) -> Result<String> {
    use crate::core::api::GroqClient;
    use crate::core::state::Message;

    let transcript = MeetingManager::get_transcript(meeting_id)
        .ok_or_else(|| anyhow::anyhow!("No transcript found for meeting {}", meeting_id))?;

    if transcript.is_empty() {
        anyhow::bail!("Transcript is empty for meeting {}", meeting_id);
    }

    let client = GroqClient::new(api_key, model);

    let system_prompt =
        "You are a meeting analyst. Analyze the following meeting transcript and provide:\n\
        1. **Summary** — brief overview of the meeting\n\
        2. **Key Decisions** — list of decisions made\n\
        3. **Action Items** — tasks assigned with responsible parties if mentioned\n\
        4. **Follow-ups** — topics that need further discussion\n\
        Be concise and structured. Use markdown formatting.";

    let messages = vec![Message {
        role: "user".to_string(),
        content: format!("Analyze this meeting transcript:\n\n{}", transcript),
    }];

    let response = client.chat_with_image(&messages, system_prompt, None)?;

    // Save analysis
    MeetingManager::save_analysis(meeting_id, &response)?;

    // Update meeting status
    if let Ok(mut meeting) = MeetingManager::load(meeting_id) {
        meeting.status = MeetingStatus::Analyzed;
        MeetingManager::save(&meeting)?;
    }

    Ok(response)
}

/// Get the path where WAV chunks are stored during transcription
pub fn chunks_dir(meeting_id: &str) -> PathBuf {
    MeetingManager::meeting_dir(meeting_id).join("chunks")
}

/// Clean up chunk files after transcription
pub fn cleanup_chunks(meeting_id: &str) -> Result<()> {
    let dir = chunks_dir(meeting_id);
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    Ok(())
}
