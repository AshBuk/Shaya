// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Live Assistant runtime model.

use crate::core::state::{Message, Role};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const NO_SUGGESTION: &str = "NO_SUGGESTION";
pub const MAX_PENDING_CHUNKS: u64 = 6;

pub const LIVE_SYSTEM_PROMPT: &str = r#"You are Live Assistant, helping the user during a live meeting.

Use the latest live transcript as context. Respond only when it clearly contains a direct question, concern, decision point, or technical topic where the user likely needs a concise answer.

When you respond, write what the user can say out loud: first person, conversational, and usually 1-3 short sentences. Do not use long checklists unless the question explicitly asks for structure.

If there is no clearly useful answer to give, return exactly:
NO_SUGGESTION"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveRequestRange {
    pub from_seq: u64,
    pub to_seq: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    pub seq: u64,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub text: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LiveTranscriptBuffer {
    segments: Vec<TranscriptSegment>,
}

impl LiveTranscriptBuffer {
    pub fn append(&mut self, segment: TranscriptSegment) {
        if segment.text.trim().is_empty() || self.segments.iter().any(|s| s.seq == segment.seq) {
            return;
        }
        self.segments.push(segment);
        self.segments.sort_by_key(|s| s.seq);
    }

    pub fn max_seq(&self) -> u64 {
        self.segments.iter().map(|s| s.seq).max().unwrap_or(0)
    }

    pub fn pending_range_after(&self, last_evaluated_seq: u64) -> Option<LiveRequestRange> {
        let from_seq = self
            .segments
            .iter()
            .find(|s| s.seq > last_evaluated_seq)
            .map(|s| s.seq)?;
        Some(LiveRequestRange {
            from_seq,
            to_seq: self.max_seq(),
        })
    }

    pub fn compact_text(&self, range: LiveRequestRange) -> String {
        self.segments
            .iter()
            .filter(|s| s.seq >= range.from_seq && s.seq <= range.to_seq)
            .map(|s| s.text.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn latest_lines(&self, limit: usize) -> String {
        let mut lines = self
            .segments
            .iter()
            .rev()
            .take(limit)
            .map(|s| format!("[{}] {}", s.seq, s.text.trim()))
            .collect::<Vec<_>>();
        lines.reverse();
        lines.join("\n")
    }
}

#[derive(Debug, Clone)]
pub struct LiveSession {
    pub chunk_secs: u64,
    pub transcript: LiveTranscriptBuffer,
    pub last_evaluated_seq: u64,
    pub in_flight: Option<LiveRequestRange>,
    pub last_speech_seq: Option<u64>,
    pub pending_utterance: Vec<String>,
    pub pending_from_seq: Option<u64>,
    pub pending_to_seq: Option<u64>,
    pub pending_started_at: Option<DateTime<Utc>>,
    pub pending_ended_at: Option<DateTime<Utc>>,
}

impl LiveSession {
    pub fn new(chunk_secs: u64) -> Self {
        Self {
            chunk_secs,
            transcript: LiveTranscriptBuffer::default(),
            last_evaluated_seq: 0,
            in_flight: None,
            last_speech_seq: None,
            pending_utterance: Vec::new(),
            pending_from_seq: None,
            pending_to_seq: None,
            pending_started_at: None,
            pending_ended_at: None,
        }
    }

    pub fn append_to_pending(
        &mut self,
        seq: u64,
        ts_start: DateTime<Utc>,
        ts_end: DateTime<Utc>,
        text: &str,
    ) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        if self.pending_from_seq.is_none() {
            self.pending_from_seq = Some(seq);
            self.pending_started_at = Some(ts_start);
        }
        self.pending_to_seq = Some(seq);
        self.pending_ended_at = Some(ts_end);
        self.pending_utterance.push(text.to_string());
    }

    pub fn has_pending(&self) -> bool {
        self.pending_utterance.iter().any(|s| !s.trim().is_empty())
    }

    pub fn pending_span(&self) -> Option<u64> {
        let from_seq = self.pending_from_seq?;
        let to_seq = self.pending_to_seq?;
        Some(to_seq.saturating_sub(from_seq) + 1)
    }

    pub fn take_pending_segment(&mut self, closing_seq: u64) -> Option<TranscriptSegment> {
        if self.pending_utterance.is_empty() {
            return None;
        }

        let text = normalize_utterance(&self.pending_utterance);
        let started_at = self.pending_started_at;
        let ended_at = self.pending_ended_at.unwrap_or_else(Utc::now);
        self.clear_pending();

        if text.is_empty() {
            return None;
        }

        Some(TranscriptSegment {
            seq: closing_seq,
            started_at: started_at?,
            ended_at,
            text,
        })
    }

    fn clear_pending(&mut self) {
        self.pending_utterance.clear();
        self.pending_from_seq = None;
        self.pending_to_seq = None;
        self.pending_started_at = None;
        self.pending_ended_at = None;
    }

    pub fn pending_range(&self) -> Option<LiveRequestRange> {
        let range = self
            .transcript
            .pending_range_after(self.last_evaluated_seq)?;
        if self.in_flight.is_some_and(|in_flight| in_flight == range) {
            return None;
        }
        Some(range)
    }

    pub fn mark_evaluated(&mut self, range: LiveRequestRange) {
        self.last_evaluated_seq = self.last_evaluated_seq.max(range.to_seq);
        if self.in_flight == Some(range) {
            self.in_flight = None;
        }
    }
}

/// Releases chunk results in recording order. STT requests finish in any order, and a
/// chunk applied early would put its words before the ones spoken earlier.
#[derive(Debug)]
pub struct ChunkQueue<T> {
    next_seq: u64,
    ready: BTreeMap<u64, T>,
}

impl<T> ChunkQueue<T> {
    pub fn new(first_seq: u64) -> Self {
        Self {
            next_seq: first_seq,
            ready: BTreeMap::new(),
        }
    }

    /// Store the result for `seq` and return every result that is now in order.
    /// Each seq must be pushed once, a skipped chunk too, or the ones after it wait.
    pub fn push(&mut self, seq: u64, item: T) -> Vec<T> {
        if seq >= self.next_seq {
            self.ready.insert(seq, item);
        }
        let mut released = Vec::new();
        while let Some(item) = self.ready.remove(&self.next_seq) {
            released.push(item);
            self.next_seq += 1;
        }
        released
    }
}

impl<T> Default for ChunkQueue<T> {
    fn default() -> Self {
        Self::new(1)
    }
}

fn normalize_utterance(parts: &[String]) -> String {
    parts
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn live_transcript_message(
    content: String,
    image: Option<crate::core::state::ImageData>,
) -> Message {
    Message {
        role: Role::User,
        content: format!("Live transcript:\n{}", content.trim()),
        image,
    }
}

pub fn is_no_suggestion(response: &str) -> bool {
    response.trim().starts_with(NO_SUGGESTION)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_queue_releases_results_in_recording_order() {
        let mut queue = ChunkQueue::new(1);

        assert!(queue.push(2, "two").is_empty());
        assert!(queue.push(3, "three").is_empty());
        assert_eq!(queue.push(1, "one"), ["one", "two", "three"]);
        assert_eq!(queue.push(4, "four"), ["four"]);
    }

    fn session() -> LiveSession {
        LiveSession::new(4)
    }

    fn segment(seq: u64, text: &str) -> TranscriptSegment {
        TranscriptSegment {
            seq,
            started_at: Utc::now(),
            ended_at: Utc::now(),
            text: text.to_string(),
        }
    }

    #[test]
    fn transcript_is_ordered_by_sequence() {
        let mut buffer = LiveTranscriptBuffer::default();
        buffer.append(segment(2, "second"));
        buffer.append(segment(1, "first"));

        let range = buffer.pending_range_after(0).unwrap();
        assert_eq!(buffer.compact_text(range), "first\nsecond");
    }

    #[test]
    fn no_suggestion_is_exact_internal_sentinel() {
        assert!(is_no_suggestion("NO_SUGGESTION"));
        assert!(is_no_suggestion(" NO_SUGGESTION\n"));
        assert!(is_no_suggestion(
            "NO_SUGGESTION\n\n(This is not a question)"
        ));
        assert!(!is_no_suggestion("No suggestion."));
        assert!(!is_no_suggestion("The answer is NO_SUGGESTION here"));
    }

    #[test]
    fn duplicate_in_flight_range_is_ignored() {
        let mut session = session();
        session.transcript.append(segment(1, "question"));
        let range = session.pending_range().unwrap();
        session.in_flight = Some(range);

        assert_eq!(session.pending_range(), None);
    }

    #[test]
    fn stale_in_flight_reschedules_one_expanded_range() {
        let mut session = session();
        session.transcript.append(segment(1, "first question"));
        let stale_range = session.pending_range().unwrap();
        session.in_flight = Some(stale_range);

        session.transcript.append(segment(2, "newer question"));
        session.in_flight = None;
        let expanded_range = session.pending_range().unwrap();

        assert_eq!(
            expanded_range,
            LiveRequestRange {
                from_seq: 1,
                to_seq: 2,
            }
        );
        session.in_flight = Some(expanded_range);
        assert_eq!(session.pending_range(), None);
    }

    #[test]
    fn append_to_pending_two_chunks_joins_with_space() {
        let mut session = session();
        let now = Utc::now();
        session.append_to_pending(1, now, now, "Can you explain");
        session.append_to_pending(2, now, now, "ownership?");

        let segment = session.take_pending_segment(2).unwrap();

        assert_eq!(segment.text, "Can you explain ownership?");
        assert_eq!(segment.seq, 2);
    }

    #[test]
    fn normalize_utterance_collapses_whitespace() {
        let parts = vec![
            "  Can   you  ".to_string(),
            "".to_string(),
            " explain\nownership? ".to_string(),
        ];

        assert_eq!(normalize_utterance(&parts), "Can you explain ownership?");
    }

    #[test]
    fn take_pending_segment_returns_none_when_empty() {
        let mut session = session();

        assert!(session.take_pending_segment(1).is_none());
    }

    #[test]
    fn take_pending_segment_resets_state() {
        let mut session = session();
        let now = Utc::now();
        session.append_to_pending(1, now, now, "question");

        assert!(session.take_pending_segment(1).is_some());

        assert!(!session.has_pending());
        assert_eq!(session.pending_from_seq, None);
        assert_eq!(session.pending_to_seq, None);
        assert_eq!(session.pending_started_at, None);
        assert_eq!(session.pending_ended_at, None);
    }

    #[test]
    fn pending_span_tracks_chunk_range() {
        let mut session = session();
        let now = Utc::now();
        session.append_to_pending(3, now, now, "first");
        session.append_to_pending(5, now, now, "third");

        assert_eq!(session.pending_span(), Some(3));
    }
}
