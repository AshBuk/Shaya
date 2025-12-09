// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Screenshot capture for Wayland/X11
//!
//! Stealth behavior:
//! - Hyprland: ShyQuiet invisible with `layerrule = no_screen_share` (see STEALTH_GUIDE.md)
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

    if is_wayland() && has_command("slurp") && has_command("grim") {
        // Wayland with wlr-layer-shell (Sway, Hyprland)
        // slurp creates layer-shell Overlay → stealth-safe
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
fn capture_area_gnome(output: &Path) -> Result<(), String> {
    let status = Command::new("gnome-screenshot")
        .args(["-a", "-f", output.to_string_lossy().as_ref()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .map_err(|e| format!("Failed to run gnome-screenshot: {}", e))?;

    if !status.success() {
        return Err("Area selection cancelled".to_string());
    }

    Ok(())
}

fn is_wayland() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok()
}

fn has_command(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn take_wayland_screenshot(output: &Path) -> Result<(), String> {
    // Try grim first (wlroots - Sway/Hyprland)
    if try_command("grim", &[output.to_string_lossy().as_ref()]) {
        return Ok(());
    }

    // Try gnome-screenshot (GNOME)
    if try_command(
        "gnome-screenshot",
        &["-f", output.to_string_lossy().as_ref()],
    ) {
        return Ok(());
    }

    // Try spectacle (KDE)
    if try_command(
        "spectacle",
        &["-b", "-n", "-o", output.to_string_lossy().as_ref()],
    ) {
        return Ok(());
    }

    Err("No screenshot tool found. Install: grim (Sway), gnome-screenshot (GNOME), or spectacle (KDE)".to_string())
}

fn take_x11_screenshot(output: &Path) -> Result<(), String> {
    // Try gnome-screenshot
    if try_command(
        "gnome-screenshot",
        &["-f", output.to_string_lossy().as_ref()],
    ) {
        return Ok(());
    }

    // Try scrot
    if try_command("scrot", &[output.to_string_lossy().as_ref()]) {
        return Ok(());
    }

    // Try spectacle
    if try_command(
        "spectacle",
        &["-b", "-n", "-o", output.to_string_lossy().as_ref()],
    ) {
        return Ok(());
    }

    Err("No screenshot tool found. Install: gnome-screenshot, scrot, or spectacle".to_string())
}

fn try_command(cmd: &str, args: &[&str]) -> bool {
    tracing::info!("Trying screenshot with: {} {:?}", cmd, args);

    match Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
    {
        Ok(status) => {
            if status.success() {
                tracing::info!("Screenshot successful with {}", cmd);
                true
            } else {
                tracing::warn!("{} failed with status: {:?}", cmd, status.code());
                false
            }
        }
        Err(e) => {
            tracing::warn!("{} not available: {}", cmd, e);
            false
        }
    }
}
