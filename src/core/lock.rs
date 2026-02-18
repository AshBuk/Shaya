// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Process lock file for single-instance without D-Bus
//!
//! STEALTH: Uses lock file instead of D-Bus activation to prevent
//! duplicate instances while keeping the app invisible in service lists.
//!
//! When a second instance is launched, it sends SIGUSR1 to the first
//! instance to show the window, then exits.

use std::fs;
use std::path::PathBuf;

/// Get lock file path
pub fn path() -> PathBuf {
    dirs::runtime_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("shaya.lock")
}

/// Acquire lock file
///
/// Returns Ok(true) if lock acquired (we are the first instance).
/// Returns Ok(false) if another instance is running (signaled to show).
/// Returns Err only on actual errors.
pub fn acquire() -> anyhow::Result<bool> {
    let lock_path = path();

    if lock_path.exists() {
        // Check if the process is still running by reading PID
        if let Ok(pid_str) = fs::read_to_string(&lock_path) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                // Check if process exists (Linux-specific)
                let proc_path = format!("/proc/{}", pid);
                if std::path::Path::new(&proc_path).exists() {
                    // Send SIGUSR1 to show the window
                    #[cfg(unix)]
                    {
                        use nix::sys::signal::{kill, Signal};
                        use nix::unistd::Pid;
                        tracing::info!(
                            "Shaya already running (PID: {}), sending show signal",
                            pid
                        );
                        if kill(Pid::from_raw(pid), Signal::SIGUSR1).is_ok() {
                            tracing::info!("Signal sent - window should appear");
                        }
                    }
                    return Ok(false); // Don't start, existing instance will show
                }
            }
        }
        // Stale lock file, remove it
        let _ = fs::remove_file(&lock_path);
    }

    // Create lock file with our PID
    fs::write(&lock_path, std::process::id().to_string())?;
    tracing::debug!("Lock acquired: {:?}", lock_path);
    Ok(true)
}

/// Release lock file
pub fn release() {
    let lock_path = path();
    if let Err(e) = fs::remove_file(&lock_path) {
        tracing::debug!("Failed to remove lock file: {}", e);
    }
}
