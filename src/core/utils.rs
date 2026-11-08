// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Shared utilities for core modules

use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

/// Check if a system command is available on PATH
pub fn command_exists(cmd: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|dir| {
            fs::metadata(dir.join(cmd))
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
    })
}

/// Create `dir` and restrict it to the current user, including when it already exists.
pub fn ensure_private_dir(dir: &Path) -> io::Result<()> {
    DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
}

/// Per-user directory for the lock file and short-lived audio and screenshots.
pub fn runtime_dir() -> io::Result<PathBuf> {
    let dir = dirs::runtime_dir()
        .or_else(dirs::cache_dir)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Neither XDG_RUNTIME_DIR nor a cache directory is available",
            )
        })?
        .join("shaya");
    ensure_private_dir(&dir)?;
    Ok(dir)
}

/// A path no other operation in this process uses, inside the private runtime directory.
pub fn temp_path(prefix: &str, extension: &str) -> io::Result<PathBuf> {
    Ok(runtime_dir()?.join(unique_name(prefix, extension)))
}

fn unique_name(prefix: &str, extension: &str) -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}_{}_{id}.{extension}", std::process::id())
}

/// Replace `path` atomically with a file only the current user can read.
/// A failure at any step leaves the previous file untouched.
pub fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Not a file path: {}", path.display()),
        ));
    };
    let tmp = parent.join(format!(
        ".{}.{}.tmp",
        name.to_string_lossy(),
        std::process::id()
    ));

    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&tmp, path)?;
        fs::File::open(parent)?.sync_all()
    })();

    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Start an audio recorder that receives SIGTERM when Shaya exits for any reason,
/// so a crash or `pkill` cannot leave it recording and it can still finalize its WAV header.
pub fn spawn_recorder(cmd: &str, args: &[String]) -> io::Result<Child> {
    let mut command = Command::new(cmd);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // SAFETY: prctl is async-signal-safe and touches no memory shared with the parent.
    #[allow(unsafe_code)]
    unsafe {
        command.pre_exec(|| {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
            Ok(())
        });
    }
    command.spawn()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn private_dir_tightens_an_existing_directory() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("meetings");
        fs::create_dir(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();

        ensure_private_dir(&dir).unwrap();

        assert_eq!(mode(&dir), 0o700);
    }

    #[test]
    fn write_private_replaces_content_with_private_mode() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("settings.json");
        fs::write(&path, "old").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

        write_private(&path, b"new").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        assert_eq!(mode(&path), 0o600);
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }

    #[test]
    fn failed_write_leaves_no_temp_file() {
        let root = tempfile::tempdir().unwrap();
        // Renaming a file over a non-empty directory always fails.
        let path = root.path().join("target");
        fs::create_dir(&path).unwrap();
        fs::write(path.join("kept"), "kept").unwrap();

        assert!(write_private(&path, b"new").is_err());

        assert_eq!(fs::read_to_string(path.join("kept")).unwrap(), "kept");
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }

    #[test]
    fn temp_names_differ_between_operations() {
        assert_ne!(unique_name("voice", "wav"), unique_name("voice", "wav"));
    }

    #[test]
    fn command_lookup_uses_path() {
        assert!(command_exists("sh"));
        assert!(!command_exists("shaya-no-such-command"));
    }
}
