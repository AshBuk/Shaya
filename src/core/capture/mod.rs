// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Screenshot capture for Wayland/X11
//! On GNOME: hides window before capture to exclude it from screenshot

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

/// Take a screenshot of the entire screen
/// Returns base64 encoded PNG image
pub fn take_screenshot() -> Result<Screenshot, String> {
    let output_path = std::env::temp_dir().join("shyquiet_screenshot.png");

    // Remove old screenshot
    let _ = fs::remove_file(&output_path);

    tracing::info!("Taking screenshot to {:?}", output_path);

    // Try different screenshot tools
    let result = if is_wayland() {
        take_wayland_screenshot(&output_path)
    } else {
        take_x11_screenshot(&output_path)
    };

    result?;

    // Read and encode the screenshot
    if !output_path.exists() {
        return Err("Screenshot file not created".to_string());
    }

    let image_data =
        fs::read(&output_path).map_err(|e| format!("Failed to read screenshot: {}", e))?;

    if image_data.len() < 100 {
        return Err("Screenshot too small".to_string());
    }

    tracing::info!("Screenshot captured: {} bytes", image_data.len());

    // Cleanup
    let _ = fs::remove_file(&output_path);

    Ok(Screenshot {
        base64_data: B64.encode(&image_data),
        mime_type: "image/png".to_string(),
    })
}

fn is_wayland() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok()
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
