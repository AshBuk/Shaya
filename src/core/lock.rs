// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Process lock file for single-instance without D-Bus
//!
//! STEALTH: Uses lock file instead of D-Bus activation to prevent
//! duplicate instances while keeping the app invisible in service lists.

use std::fs;
use std::path::PathBuf;

/// Get lock file path
pub fn path() -> PathBuf {
    dirs::runtime_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("shyquiet.lock")
}

/// Acquire lock file
///
/// Returns Ok if lock acquired, Err if already running.
/// Handles stale lock files by checking if PID is still alive.
pub fn acquire() -> anyhow::Result<()> {
    let lock_path = path();

    if lock_path.exists() {
        // Check if the process is still running by reading PID
        if let Ok(pid_str) = fs::read_to_string(&lock_path) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                // Check if process exists (Linux-specific)
                let proc_path = format!("/proc/{}", pid);
                if std::path::Path::new(&proc_path).exists() {
                    anyhow::bail!("ShyQuiet is already running (PID: {})", pid);
                }
            }
        }
        // Stale lock file, remove it
        let _ = fs::remove_file(&lock_path);
    }

    // Create lock file with our PID
    fs::write(&lock_path, std::process::id().to_string())?;
    tracing::debug!("Lock acquired: {:?}", lock_path);
    Ok(())
}

/// Release lock file
pub fn release() {
    let lock_path = path();
    if let Err(e) = fs::remove_file(&lock_path) {
        tracing::debug!("Failed to remove lock file: {}", e);
    }
}
