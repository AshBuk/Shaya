// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Meeting data model and directory-based persistence
//!
//! Each meeting is stored in its own directory under
//! `~/.local/share/shaya/meetings/meeting-{timestamp}/`

use crate::core::api::{SpeechSegment, WhisperClient};
use crate::core::audio::chunking;
use crate::core::utils;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Audio sources of a meeting, in transcript order
const SOURCES: [&str; 2] = ["system", "mic"];

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
    /// Latest recording, transcription or analysis failure, shown in the meeting list
    #[serde(default)]
    pub last_error: Option<String>,
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
            last_error: None,
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
            .join("shaya")
            .join("meetings")
    }

    /// Get directory for a specific meeting
    pub fn meeting_dir(meeting_id: &str) -> PathBuf {
        Self::base_dir().join(meeting_id)
    }

    /// Create a meeting directory only the current user can enter
    pub fn prepare_meeting_dir(meeting_id: &str) -> io::Result<PathBuf> {
        Self::protect_base_dir()?;
        let dir = Self::meeting_dir(meeting_id);
        utils::ensure_private_dir(&dir)?;
        Ok(dir)
    }

    fn protect_base_dir() -> io::Result<()> {
        let base = Self::base_dir();
        if let Some(app_dir) = base.parent() {
            utils::ensure_private_dir(app_dir)?;
        }
        utils::ensure_private_dir(&base)
    }

    /// Startup repair, before any recording can run: restrict access to recordings
    /// from older versions and release meetings an interrupted run left mid-operation,
    /// which the list offers no action for.
    pub fn prepare_storage() {
        if let Err(e) = Self::protect_base_dir() {
            tracing::warn!("Failed to protect meeting storage: {}", e);
        }
        for mut meeting in Self::list() {
            if matches!(
                meeting.status,
                MeetingStatus::Recording | MeetingStatus::Converting | MeetingStatus::Transcribing
            ) {
                meeting.status = MeetingStatus::Completed;
                meeting.last_error = Some("Interrupted when Shaya closed".to_string());
                if let Err(e) = Self::save(&meeting) {
                    tracing::warn!("Failed to repair meeting {}: {}", meeting.id, e);
                }
            }
        }
    }

    /// Save meeting metadata to its directory
    pub fn save(meeting: &Meeting) -> Result<()> {
        let dir = Self::prepare_meeting_dir(&meeting.id)?;
        let content = serde_json::to_string_pretty(meeting)?;
        utils::write_private(&dir.join("metadata.json"), content.as_bytes())?;
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
        let path = Self::prepare_meeting_dir(meeting_id)?.join("transcript.txt");
        utils::write_private(&path, transcript.as_bytes())?;
        Ok(())
    }

    /// Get analysis text for a meeting (if available)
    pub fn get_analysis(meeting_id: &str) -> Option<String> {
        let path = Self::meeting_dir(meeting_id).join("analysis.md");
        fs::read_to_string(&path).ok()
    }

    /// Save analysis text for a meeting. Fails if the meeting was deleted while the
    /// request was running: creating its directory again would leave a file no list shows.
    pub fn save_analysis(meeting_id: &str, analysis: &str) -> Result<()> {
        let path = Self::meeting_dir(meeting_id).join("analysis.md");
        utils::write_private(&path, analysis.as_bytes())?;
        Ok(())
    }

    /// Find any audio file for playback (prefers OPUS, falls back to WAV)
    pub fn find_playback_file(meeting_id: &str) -> Option<PathBuf> {
        let dir = Self::meeting_dir(meeting_id);
        [
            "mixed.opus",
            "mixed.wav",
            "system.opus",
            "system.wav",
            "mic.opus",
            "mic.wav",
        ]
        .iter()
        .map(|name| dir.join(name))
        .find(|p| p.exists())
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

/// Pick the audio file of each source separately: WAV when it exists (it can be
/// split to fit the upload limit), otherwise the OPUS copy.
fn transcribable_files_in(dir: &Path) -> Vec<PathBuf> {
    SOURCES
        .iter()
        .filter_map(|source| {
            ["wav", "opus"]
                .iter()
                .map(|extension| dir.join(format!("{source}.{extension}")))
                .find(|path| path.exists())
        })
        .collect()
}

/// Transcribe a meeting's audio files. Any failed part fails the whole operation, so the
/// user can retry: parts that already succeeded are kept on disk and not billed twice.
/// Call this from a blocking thread (not the GTK main loop).
pub fn transcribe_meeting(
    meeting_id: &str,
    stt_provider: crate::core::state::SttProvider,
    stt_api_key: &str,
    stt_model: &str,
) -> Result<String> {
    let meeting_dir = MeetingManager::prepare_meeting_dir(meeting_id)?;
    let audio_files = transcribable_files_in(&meeting_dir);
    if audio_files.is_empty() {
        anyhow::bail!("No audio files found for meeting {}", meeting_id);
    }

    let parts_dir = chunks_dir(meeting_id);
    utils::ensure_private_dir(&parts_dir)?;
    let mut parts = Vec::new();
    for path in &audio_files {
        let label = source_label(path);
        if path.extension().is_some_and(|ext| ext == "wav") {
            for (index, chunk) in chunking::chunk_wav(path, &parts_dir)?
                .into_iter()
                .enumerate()
            {
                parts.push(Part {
                    path: chunk,
                    label,
                    offset_secs: (index as u64 * chunking::DEFAULT_CHUNK_SECS) as f64,
                });
            }
        } else {
            parts.push(Part {
                path: path.clone(),
                label,
                offset_secs: 0.0,
            });
        }
    }

    tracing::info!("Transcribing {} parts for {}", parts.len(), meeting_id);
    let client = WhisperClient::new(stt_provider, stt_api_key, stt_model);
    let multi_source = audio_files.len() > 1;
    // Timestamps interleave the sources; without them a part is ordered by its start.
    let timed = multi_source && client.has_timestamps();
    let transcript = transcribe_parts(&parts, &parts_dir, multi_source, |path| {
        if timed {
            client.transcribe_segments(path)
        } else {
            client
                .transcribe(path)
                .map(|text| vec![SpeechSegment { start: 0.0, text }])
        }
    })?;

    MeetingManager::save_transcript(meeting_id, &transcript)?;
    cleanup_chunks(meeting_id)?;
    remove_converted_wavs(&meeting_dir);
    if let Ok(mut meeting) = MeetingManager::load(meeting_id) {
        meeting.status = MeetingStatus::Transcribed;
        meeting.last_error = None;
        MeetingManager::save(&meeting)?;
    }

    tracing::info!(
        "Transcription complete for {} ({} chars)",
        meeting_id,
        transcript.len()
    );
    Ok(transcript)
}

/// One uploaded piece of a source recording.
struct Part {
    path: PathBuf,
    label: &'static str,
    /// Where the part starts in its source recording.
    offset_secs: f64,
}

/// Speech placed on the meeting timeline.
struct Spoken<'a> {
    at: f64,
    part: usize,
    label: &'a str,
    text: String,
}

/// Transcribe every part, caching each result next to the parts.
fn transcribe_parts(
    parts: &[Part],
    cache_dir: &Path,
    multi_source: bool,
    mut transcribe: impl FnMut(&Path) -> Result<Vec<SpeechSegment>>,
) -> Result<String> {
    let mut spoken = Vec::new();
    let mut failures = Vec::new();

    for (index, part) in parts.iter().enumerate() {
        let stem = part.path.file_stem().unwrap_or_default().to_string_lossy();
        let cache = cache_dir.join(format!("{stem}.json"));
        let cached = fs::read(&cache)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Vec<SpeechSegment>>(&bytes).ok());
        let segments = match cached {
            Some(segments) => segments,
            None => match transcribe(&part.path) {
                Ok(segments) => {
                    utils::write_private(&cache, &serde_json::to_vec(&segments)?)?;
                    segments
                }
                Err(e) => {
                    tracing::warn!("Transcription error for {}: {}", part.path.display(), e);
                    failures.push(e.to_string());
                    continue;
                }
            },
        };
        spoken.extend(segments.into_iter().map(|segment| Spoken {
            at: part.offset_secs + segment.start,
            part: index,
            label: part.label,
            text: segment.text,
        }));
    }

    if let Some(first) = failures.first() {
        anyhow::bail!(
            "{} of {} parts failed, retry to continue: {}",
            failures.len(),
            parts.len(),
            first
        );
    }
    Ok(format_transcript(spoken, multi_source))
}

/// Order speech by time. A new line, labelled when there are several sources,
/// starts wherever the speech moves to another part.
fn format_transcript(mut spoken: Vec<Spoken>, multi_source: bool) -> String {
    // Stable: segments at the same second keep their part order.
    spoken.sort_by(|a, b| a.at.total_cmp(&b.at));
    let mut transcript = String::new();
    let mut current_part = None;
    for s in &spoken {
        let text = s.text.trim();
        if text.is_empty() {
            continue;
        }
        if current_part == Some(s.part) {
            transcript.push(' ');
        } else {
            if !transcript.is_empty() {
                transcript.push('\n');
            }
            if multi_source {
                transcript.push_str(s.label);
                transcript.push(' ');
            }
            current_part = Some(s.part);
        }
        transcript.push_str(text);
    }
    transcript
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

/// Transcription no longer needs the large WAVs that have a compressed copy.
fn remove_converted_wavs(dir: &Path) {
    for source in SOURCES {
        if dir.join(format!("{source}.opus")).exists() {
            let _ = fs::remove_file(dir.join(format!("{source}.wav")));
        }
    }
}

/// Analyze a meeting transcript using AI.
/// Call this from a blocking thread.
pub fn analyze_meeting(
    meeting_id: &str,
    provider: crate::core::state::AiProvider,
    api_key: &str,
    model: &str,
) -> Result<String> {
    use crate::core::api::LlmClient;
    use crate::core::state::{Message, Role};

    let transcript = MeetingManager::get_transcript(meeting_id)
        .ok_or_else(|| anyhow::anyhow!("No transcript found for meeting {}", meeting_id))?;

    if transcript.is_empty() {
        anyhow::bail!("Transcript is empty for meeting {}", meeting_id);
    }

    let client = LlmClient::new(provider, api_key, model);

    let system_prompt =
        "You are a meeting analyst. Analyze the following meeting transcript and provide:\n\
        1. **Summary** — brief overview of the meeting\n\
        2. **Key Decisions** — list of decisions made\n\
        3. **Action Items** — tasks assigned with responsible parties if mentioned\n\
        4. **Follow-ups** — topics that need further discussion\n\
        Be concise and structured. Use markdown formatting.";

    let messages = vec![Message {
        role: Role::User,
        content: format!("Analyze this meeting transcript:\n\n{}", transcript),
        image: None,
    }];

    let response = client.chat(&messages, system_prompt)?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn part(path: &Path, label: &'static str, offset_secs: f64) -> Part {
        Part {
            path: path.to_path_buf(),
            label,
            offset_secs,
        }
    }

    fn said(start: f64, text: &str) -> Vec<SpeechSegment> {
        vec![SpeechSegment {
            start,
            text: text.to_string(),
        }]
    }

    fn touch(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, "audio").unwrap();
        path
    }

    #[test]
    fn analysis_of_a_deleted_meeting_is_not_saved() {
        let id = format!("meeting-deleted-{}", std::process::id());

        assert!(MeetingManager::save_analysis(&id, "analysis").is_err());
        assert!(!MeetingManager::meeting_dir(&id).exists());
    }

    #[test]
    fn each_source_picks_its_own_format() {
        let dir = tempfile::tempdir().unwrap();
        let system = touch(dir.path(), "system.opus");
        let mic = touch(dir.path(), "mic.wav");

        assert_eq!(transcribable_files_in(dir.path()), vec![system, mic]);
    }

    #[test]
    fn wav_wins_over_its_opus_copy() {
        let dir = tempfile::tempdir().unwrap();
        let wav = touch(dir.path(), "system.wav");
        touch(dir.path(), "system.opus");

        assert_eq!(transcribable_files_in(dir.path()), vec![wav]);
    }

    #[test]
    fn failed_part_fails_the_operation_and_retry_only_repeats_it() {
        let dir = tempfile::tempdir().unwrap();
        let first = touch(dir.path(), "system_chunk_000.wav");
        let second = touch(dir.path(), "system_chunk_001.wav");
        let parts = [
            part(&first, "[System]", 0.0),
            part(&second, "[System]", 600.0),
        ];
        let calls = RefCell::new(Vec::new());

        let failed = transcribe_parts(&parts, dir.path(), false, |path| {
            calls.borrow_mut().push(path.to_path_buf());
            if path == second {
                anyhow::bail!("rate limit")
            }
            Ok(said(0.0, "one"))
        });
        assert!(failed.unwrap_err().to_string().contains("1 of 2"));

        let transcript = transcribe_parts(&parts, dir.path(), false, |path| {
            calls.borrow_mut().push(path.to_path_buf());
            Ok(said(0.0, "two"))
        })
        .unwrap();

        assert_eq!(transcript, "one\ntwo");
        assert_eq!(*calls.borrow(), vec![first, second.clone(), second]);
    }

    #[test]
    fn sources_are_interleaved_by_time() {
        let dir = tempfile::tempdir().unwrap();
        let system = touch(dir.path(), "system.wav");
        let mic = touch(dir.path(), "mic.wav");
        let parts = [part(&system, "[System]", 0.0), part(&mic, "[Mic]", 0.0)];

        let transcript = transcribe_parts(&parts, dir.path(), true, |path| {
            Ok(if path == system {
                [
                    said(0.0, "How are you?"),
                    said(4.0, "Good."),
                    said(9.0, "Bye."),
                ]
                .concat()
            } else {
                [
                    said(2.0, "Fine, you?"),
                    said(3.0, " "),
                    said(6.0, "See you."),
                ]
                .concat()
            })
        })
        .unwrap();

        assert_eq!(
            transcript,
            "[System] How are you?\n[Mic] Fine, you?\n[System] Good.\n[Mic] See you.\n[System] Bye."
        );
    }

    #[test]
    fn only_wavs_with_a_compressed_copy_are_removed() {
        let dir = tempfile::tempdir().unwrap();
        let system = touch(dir.path(), "system.wav");
        touch(dir.path(), "system.opus");
        let mic = touch(dir.path(), "mic.wav");

        remove_converted_wavs(dir.path());

        assert!(!system.exists());
        assert!(mic.exists());
    }
}
