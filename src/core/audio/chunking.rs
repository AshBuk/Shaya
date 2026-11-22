// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! WAV file chunking for Whisper API (25MB limit)
//!
//! At 16kHz mono s16, WAV = ~32KB/sec = ~1.9MB/min.
//! The 25MB limit = ~13 minutes of WAV.
//! We chunk into 10-minute segments to stay well under the limit.

use anyhow::Result;
use std::path::{Path, PathBuf};

/// Default chunk duration in seconds (10 minutes)
pub const DEFAULT_CHUNK_SECS: u64 = 600;

/// Split a WAV file into time-based chunks.
/// Returns paths to chunk files. If the file is short enough, returns the original path.
pub fn chunk_wav(wav_path: &Path, output_dir: &Path) -> Result<Vec<PathBuf>> {
    chunk_wav_with_duration(wav_path, output_dir, DEFAULT_CHUNK_SECS)
}

/// Split a WAV file into chunks of specified duration.
pub fn chunk_wav_with_duration(
    wav_path: &Path,
    output_dir: &Path,
    chunk_duration_secs: u64,
) -> Result<Vec<PathBuf>> {
    let reader = hound::WavReader::open(wav_path)?;
    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let samples_per_chunk = sample_rate as u64 * chunk_duration_secs * spec.channels as u64;
    let total_samples = reader.len() as u64;

    // If file fits in one chunk, return original path (no splitting needed)
    if total_samples <= samples_per_chunk {
        tracing::info!(
            "WAV file fits in one chunk ({} samples), no splitting needed",
            total_samples
        );
        return Ok(vec![wav_path.to_path_buf()]);
    }

    std::fs::create_dir_all(output_dir)?;

    let num_chunks = total_samples.div_ceil(samples_per_chunk);
    tracing::info!(
        "Splitting WAV into {} chunks of {}s each",
        num_chunks,
        chunk_duration_secs
    );

    let mut chunk_paths = Vec::new();
    let mut samples = reader.into_samples::<i16>();
    let stem = wav_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("chunk");

    for chunk_idx in 0..num_chunks {
        let chunk_path = output_dir.join(format!("{}_chunk_{:03}.wav", stem, chunk_idx));
        let mut writer = hound::WavWriter::create(&chunk_path, spec)?;

        let mut written = 0u64;
        while written < samples_per_chunk {
            match samples.next() {
                Some(Ok(sample)) => {
                    writer.write_sample(sample)?;
                    written += 1;
                }
                Some(Err(e)) => {
                    tracing::warn!("Error reading sample in chunk {}: {}", chunk_idx, e);
                    break;
                }
                None => break,
            }
        }

        writer.finalize()?;

        if written > 0 {
            tracing::info!(
                "Chunk {}/{}: {} samples written to {}",
                chunk_idx + 1,
                num_chunks,
                written,
                chunk_path.display()
            );
            chunk_paths.push(chunk_path);
        }
    }

    Ok(chunk_paths)
}
