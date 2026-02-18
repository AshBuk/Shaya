// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Screenshot capture for Wayland/X11
//!
//! Stealth behavior:
//! - Hyprland: ShyQuiet invisible with `layerrule = noscreenshare` (see STEALTH_GUIDE.md)
//! - Sway: ShyQuiet WILL appear in screenshots (no layer filtering support)
//! - Selection UI (slurp overlay) is always invisible during screen share

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

/// Screenshot result with base64 encoded image
#[derive(Clone)]
pub struct Screenshot {
    pub base64_data: String,
    pub mime_type: String,
}

/// Capture user-selected area
/// Uses slurp + grim on Wayland, gnome-screenshot -a on GNOME/X11
pub fn capture_area() -> Result<Screenshot, String> {
    let output_path = std::env::temp_dir().join("shyquiet_area.png");
    let _ = fs::remove_file(&output_path);

    tracing::info!("Starting area selection");

    if is_wayland() && !is_gnome() && has_command("slurp") && has_command("grim") {
        // wlroots compositors only (Sway, Hyprland) — slurp and grim both require
        // wlr-layer-shell / wlr-screencopy protocols that GNOME Wayland does not expose.
        capture_area_slurp_grim(&output_path)?;
    } else if has_command("gnome-screenshot") {
        // GNOME or X11 fallback
        capture_area_gnome(&output_path)?;
    } else {
        return Err("No area capture tool. Install: slurp+grim or gnome-screenshot".to_string());
    }

    read_and_encode(&output_path)
}

/// Take a screenshot of the entire screen
pub fn take_screenshot() -> Result<Screenshot, String> {
    let output_path = std::env::temp_dir().join("shyquiet_screenshot.png");
    let _ = fs::remove_file(&output_path);

    tracing::info!("Taking full screenshot");

    let result = if is_wayland() {
        take_wayland_screenshot(&output_path)
    } else {
        take_x11_screenshot(&output_path)
    };

    result?;
    read_and_encode(&output_path)
}

/// Read screenshot file and encode to base64
fn read_and_encode(path: &Path) -> Result<Screenshot, String> {
    if !path.exists() {
        return Err("Screenshot file not created".to_string());
    }

    let image_data = fs::read(path).map_err(|e| format!("Failed to read screenshot: {}", e))?;

    if image_data.len() < 100 {
        return Err("Screenshot too small or cancelled".to_string());
    }

    tracing::info!("Screenshot captured: {} bytes", image_data.len());

    let _ = fs::remove_file(path);

    Ok(Screenshot {
        base64_data: B64.encode(&image_data),
        mime_type: "image/png".to_string(),
    })
}

/// Capture area using slurp + grim (stealth-safe on Sway/Hyprland)
fn capture_area_slurp_grim(output: &Path) -> Result<(), String> {
    // Run slurp to get geometry (layer-shell Overlay → invisible in screen share)
    let slurp_output = Command::new("slurp")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to run slurp: {}", e))?;

    if !slurp_output.status.success() {
        return Err("Area selection cancelled".to_string());
    }

    let geometry = String::from_utf8_lossy(&slurp_output.stdout)
        .trim()
        .to_string();

    if geometry.is_empty() {
        return Err("No area selected".to_string());
    }

    tracing::info!("Selected area: {}", geometry);

    // Capture with grim
    let status = Command::new("grim")
        .args(["-g", &geometry, output.to_string_lossy().as_ref()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .map_err(|e| format!("Failed to run grim: {}", e))?;

    if !status.success() {
        return Err("grim capture failed".to_string());
    }

    Ok(())
}

/// Capture area using gnome-screenshot -a
///
/// On X11 / older GNOME, `-f path` is honoured directly.
/// On GNOME Wayland (44+) the `-f` flag is silently ignored and the file is
/// saved to ~/Pictures/Screenshots/ instead — we find it there and move it.
fn capture_area_gnome(output: &Path) -> Result<(), String> {
    let status = Command::new("gnome-screenshot")
        .args(["-a", "-f", output.to_string_lossy().as_ref()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("Failed to run gnome-screenshot: {}", e))?;

    if !status.success() {
        return Err("Area selection cancelled".to_string());
    }

    // X11 / old GNOME: file written directly to output path
    if output.exists() {
        return Ok(());
    }

    // GNOME Wayland: find the file saved to the default screenshots directory
    tracing::info!(
        "gnome-screenshot ignored -f flag (GNOME Wayland); searching in Pictures/Screenshots"
    );
    match find_latest_gnome_screenshot() {
        Some(path) => {
            fs::rename(&path, output).map_err(|e| format!("Failed to move screenshot: {}", e))
        }
        None => Err(
            "Area screenshot not found. Ensure ~/Pictures/Screenshots/ exists and is writable"
                .to_string(),
        ),
    }
}

/// Find the most recently created PNG in ~/Pictures/Screenshots (GNOME Wayland default).
/// Only considers files modified within the last 10 seconds to avoid picking old ones.
fn find_latest_gnome_screenshot() -> Option<std::path::PathBuf> {
    let dir = dirs::picture_dir()?.join("Screenshots");
    let cutoff = std::time::SystemTime::now().checked_sub(std::time::Duration::from_secs(10))?;

    fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "png").unwrap_or(false))
        .filter(|e| {
            e.metadata()
                .and_then(|m| m.modified())
                .map(|t| t > cutoff)
                .unwrap_or(false)
        })
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())
        .map(|e| e.path())
}

fn is_wayland() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok()
}

fn is_gnome() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_lowercase()
        .contains("gnome")
}

fn has_command(cmd: &str) -> bool {
    crate::core::utils::command_exists(cmd)
}

fn take_wayland_screenshot(output: &Path) -> Result<(), String> {
    // GNOME Wayland: org.gnome.Shell.Screenshot is restricted to trusted processes
    // (AccessDenied for third-party apps since GNOME 44). Use gnome-screenshot which
    // invokes the portal internally — it ignores -f and saves to ~/Pictures/Screenshots/,
    // so we find it there afterwards (same strategy as capture_area_gnome).
    if is_gnome() {
        return take_gnome_wayland_screenshot(output);
    }

    // wlroots compositors (Sway, Hyprland)
    if try_capture_to_file("grim", &[&output.to_string_lossy()], output) {
        return Ok(());
    }

    // KDE Plasma Wayland
    if try_capture_to_file(
        "spectacle",
        &["-b", "-n", "-o", &output.to_string_lossy()],
        output,
    ) {
        return Ok(());
    }

    Err("No screenshot tool found. Install: grim (Sway/Hyprland) or spectacle (KDE)".to_string())
}

/// Full screenshot on GNOME Wayland.
///
/// The correct path is the XDG Desktop Portal (`org.freedesktop.portal.Screenshot`),
/// which is available but async: the call returns a D-Bus request handle and the
/// actual file URI arrives via a `Response` signal. Implementing this requires
/// async GIO D-Bus, which does not fit the current sync capture architecture.
///
/// TODO: implement via portal using glib::spawn_future_local + GIO async D-Bus.
fn take_gnome_wayland_screenshot(_output: &Path) -> Result<(), String> {
    Err(
        "Screenshots on GNOME Wayland require XDG Desktop Portal integration \
         (not yet implemented). Use Sway or Hyprland with grim for full screenshot \
         support, or take a screenshot manually and describe what you see."
            .to_string(),
    )
}

fn take_x11_screenshot(output: &Path) -> Result<(), String> {
    // gnome-screenshot
    if try_capture_to_file(
        "gnome-screenshot",
        &["-f", &output.to_string_lossy()],
        output,
    ) {
        return Ok(());
    }

    // scrot
    if try_capture_to_file("scrot", &[&output.to_string_lossy()], output) {
        return Ok(());
    }

    // spectacle
    if try_capture_to_file(
        "spectacle",
        &["-b", "-n", "-o", &output.to_string_lossy()],
        output,
    ) {
        return Ok(());
    }

    Err("No screenshot tool found. Install: gnome-screenshot, scrot, or spectacle".to_string())
}

/// Run a command and verify the output file was actually created.
/// Returns false if the command fails OR if the file does not exist afterwards.
fn try_capture_to_file(cmd: &str, args: &[&str], output: &Path) -> bool {
    tracing::info!("Trying screenshot with: {} {:?}", cmd, args);

    let ok = Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if ok && output.exists() {
        tracing::info!("Screenshot captured with {}", cmd);
        true
    } else {
        if ok {
            tracing::warn!(
                "{} exited successfully but file not created at {:?}",
                cmd,
                output
            );
        } else {
            tracing::warn!("{} failed", cmd);
        }
        false
    }
}
