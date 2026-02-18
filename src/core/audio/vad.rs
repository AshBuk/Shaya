// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Voice Activity Detection
//!
//! Post-recording analysis: detect if audio contains speech,
//! filter silence, avoid transcribing empty recordings.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Voice Activity Detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VadConfig {
    pub enabled: bool,
    pub hop_size: usize,
    pub sensitivity_rms: f32,
    pub peak_threshold: f32,
    pub silence_chunks: usize,
    pub min_speech_chunks: usize,
    pub pre_speech_chunks: usize,
    pub noise_gate_threshold: f32,
    pub max_recording_duration_secs: u64,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hop_size: 1024,
            sensitivity_rms: 0.012,
            peak_threshold: 0.035,
            silence_chunks: 45,
            min_speech_chunks: 7,
            pre_speech_chunks: 12,
            noise_gate_threshold: 0.003,
            max_recording_duration_secs: 180,
        }
    }
}

/// Calculate RMS and peak for audio chunk
pub fn calculate_audio_metrics(chunk: &[f32]) -> (f32, f32) {
    if chunk.is_empty() {
        return (0.0, 0.0);
    }

    let mut sumsq = 0.0f32;
    let mut peak = 0.0f32;

    for &v in chunk {
        let a = v.abs();
        peak = peak.max(a);
        sumsq += v * v;
    }

    let rms = (sumsq / chunk.len() as f32).sqrt();
    (rms, peak)
}

/// Result of VAD analysis
#[derive(Debug)]
pub struct VadResult {
    /// Whether speech was detected
    pub has_speech: bool,
    /// Average RMS level
    pub avg_rms: f32,
    /// Peak level
    pub peak: f32,
    /// Duration in seconds
    pub duration_secs: f32,
}

/// Analyze WAV file for voice activity
/// Returns None if file cannot be read, Some(VadResult) otherwise
pub fn analyze_audio_file(path: &Path, config: &VadConfig) -> Option<VadResult> {
    let reader = hound::WavReader::open(path).ok()?;
    let spec = reader.spec();
    let sample_rate = spec.sample_rate as f32;

    tracing::debug!(
        "VAD: Reading WAV - format={:?}, channels={}, rate={}, bits={}",
        spec.sample_format,
        spec.channels,
        spec.sample_rate,
        spec.bits_per_sample
    );

    // Read samples and convert to f32 (normalized to -1.0..1.0)
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            // Read as i16 for 16-bit WAV files
            let raw: Vec<i16> = reader
                .into_samples::<i16>()
                .filter_map(|s| s.ok())
                .collect();
            tracing::debug!("VAD: Read {} i16 samples", raw.len());
            raw.iter().map(|&s| s as f32 / i16::MAX as f32).collect()
        }
        hound::SampleFormat::Float => reader
            .into_samples::<f32>()
            .filter_map(|s| s.ok())
            .collect(),
    };

    tracing::debug!("VAD: Total {} f32 samples", samples.len());

    if samples.is_empty() {
        return Some(VadResult {
            has_speech: false,
            avg_rms: 0.0,
            peak: 0.0,
            duration_secs: 0.0,
        });
    }

    let duration_secs = samples.len() as f32 / sample_rate;

    // Analyze in chunks
    let mut speech_chunks = 0usize;
    let mut total_rms = 0.0f32;
    let mut max_peak = 0.0f32;
    let mut chunk_count = 0usize;

    for chunk in samples.chunks(config.hop_size) {
        let (rms, peak) = calculate_audio_metrics(chunk);
        total_rms += rms;
        max_peak = max_peak.max(peak);
        chunk_count += 1;

        // Check if this chunk has speech
        if rms > config.sensitivity_rms || peak > config.peak_threshold {
            speech_chunks += 1;
        }
    }

    let avg_rms = if chunk_count > 0 {
        total_rms / chunk_count as f32
    } else {
        0.0
    };

    // Speech detected if enough chunks have activity
    let has_speech = speech_chunks >= config.min_speech_chunks;

    tracing::debug!(
        "VAD: {} chunks with speech out of {}, avg_rms={:.4}, peak={:.4}",
        speech_chunks,
        chunk_count,
        avg_rms,
        max_peak
    );

    Some(VadResult {
        has_speech,
        avg_rms,
        peak: max_peak,
        duration_secs,
    })
}
