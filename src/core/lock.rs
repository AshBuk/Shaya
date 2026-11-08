// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Single-instance lock without D-Bus
//!
//! STEALTH: Uses a file lock instead of D-Bus activation to prevent
//! duplicate instances while keeping the app invisible in service lists.
//!
//! The kernel releases the lock when the process dies, so a stale file cannot
//! point at an unrelated process. A second instance sends SIGUSR1 to the PID
//! stored by the lock holder, then exits.

use crate::core::utils;
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

/// Keeps this process the running instance until dropped.
pub struct InstanceLock {
    _file: File,
}

/// Returns the lock when this is the first instance.
/// Returns `None` after signaling the instance that is already running.
pub fn acquire() -> anyhow::Result<Option<InstanceLock>> {
    let path = utils::runtime_dir()?.join("shaya.lock");
    let lock = try_acquire(&path)?;
    if lock.is_none() {
        signal_running_instance(&path);
    }
    Ok(lock)
}

fn try_acquire(path: &Path) -> io::Result<Option<InstanceLock>> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(path)?;

    match file.try_lock() {
        Ok(()) => {
            file.set_len(0)?;
            file.write_all(std::process::id().to_string().as_bytes())?;
            tracing::debug!("Lock acquired: {:?}", path);
            Ok(Some(InstanceLock { _file: file }))
        }
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(e)) => Err(e),
    }
}

fn signal_running_instance(path: &Path) {
    let pid = fs::read_to_string(path)
        .ok()
        .and_then(|content| content.trim().parse::<i32>().ok());
    let Some(pid) = pid else {
        tracing::warn!("Shaya is starting in another process, no window to toggle yet");
        return;
    };
    tracing::info!("Shaya already running (PID: {}), sending show signal", pid);
    if let Err(e) = kill(Pid::from_raw(pid), Signal::SIGUSR1) {
        tracing::warn!("Failed to signal running instance: {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_holder_at_a_time() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("shaya.lock");

        let first = try_acquire(&path).unwrap();
        assert!(first.is_some());
        assert!(try_acquire(&path).unwrap().is_none());

        drop(first);
        assert!(try_acquire(&path).unwrap().is_some());
    }

    #[test]
    fn stale_pid_from_a_dead_process_does_not_block_startup() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("shaya.lock");
        fs::write(&path, "1").unwrap();

        let lock = try_acquire(&path).unwrap();

        assert!(lock.is_some());
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            std::process::id().to_string()
        );
    }
}
