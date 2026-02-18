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
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

type PortalResponse = (u32, std::collections::HashMap<String, glib::Variant>);
type SharedPortalResult = std::rc::Rc<std::cell::RefCell<Option<Result<PortalResponse, String>>>>;

/// Screenshot result with base64 encoded image
#[derive(Clone)]
pub struct Screenshot {
    pub base64_data: String,
    pub mime_type: String,
}

/// Capture user-selected area (sync path).
/// Wayland wlroots: slurp + grim. KDE: spectacle -r. X11: gnome-screenshot -a.
/// GNOME Wayland uses the async XDG portal instead — see `take_screenshot_async`.
pub fn capture_area() -> Result<Screenshot, String> {
    if requires_async_portal() {
        return Err(
            "GNOME Wayland area capture requires the async XDG Desktop Portal; \
             use take_screenshot_async() from the UI main loop."
                .to_string(),
        );
    }

    let output_path = std::env::temp_dir().join("shyquiet_area.png");
    let _ = fs::remove_file(&output_path);

    tracing::info!("Starting area selection");

    if is_wayland() && has_command("slurp") && has_command("grim") {
        capture_area_slurp_grim(&output_path)?;
    } else if has_command("spectacle") {
        capture_area_spectacle(&output_path)?;
    } else if has_command("gnome-screenshot") {
        capture_area_gnome(&output_path)?;
    } else {
        return Err(
            "No area capture tool. Install: slurp+grim (wlroots), spectacle (KDE), or gnome-screenshot"
                .to_string(),
        );
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

/// True when GNOME Wayland portal path must be used.
pub fn requires_async_portal() -> bool {
    is_wayland() && is_gnome()
}

/// Async screenshot path for GNOME Wayland via XDG Desktop Portal.
/// Returns `None` when the user cancels or denies the portal dialog.
pub async fn take_screenshot_async() -> Result<Option<Screenshot>, String> {
    if !requires_async_portal() {
        return take_screenshot().map(Some);
    }

    let output_path = std::env::temp_dir().join("shyquiet_screenshot.png");
    let _ = fs::remove_file(&output_path);

    tracing::info!("Taking GNOME Wayland screenshot via XDG portal");
    if !take_gnome_wayland_screenshot_portal(&output_path).await? {
        return Ok(None);
    }
    read_and_encode(&output_path).map(Some)
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

/// Capture area using spectacle -r (KDE Plasma, Wayland and X11).
fn capture_area_spectacle(output: &Path) -> Result<(), String> {
    let status = Command::new("spectacle")
        .args(["-r", "-b", "-n", "-o", output.to_string_lossy().as_ref()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("Failed to run spectacle: {}", e))?;

    if !status.success() {
        return Err("Area selection cancelled".to_string());
    }
    if output.exists() {
        Ok(())
    } else {
        Err("spectacle succeeded but file was not created".to_string())
    }
}

/// Capture area using gnome-screenshot -a (X11 only).
///
/// On GNOME Wayland, the XDG Desktop Portal async path is used instead
/// (see `take_screenshot_async`), so this function targets X11 only.
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

    if output.exists() {
        Ok(())
    } else {
        Err("gnome-screenshot succeeded but file was not created".to_string())
    }
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
    if is_gnome() {
        return Err(
            "GNOME Wayland screenshots require the async XDG Desktop Portal; \
             use take_screenshot_async() from the UI main loop."
                .to_string(),
        );
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

/// Full screenshot on GNOME Wayland via XDG Desktop Portal.
/// Returns `Ok(true)` on success, `Ok(false)` when user cancelled/denied.
async fn take_gnome_wayland_screenshot_portal(output: &Path) -> Result<bool, String> {
    let proxy = gio::DBusProxy::for_bus_future(
        gio::BusType::Session,
        gio::DBusProxyFlags::NONE,
        None::<&gio::DBusInterfaceInfo>,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.Screenshot",
    )
    .await
    .map_err(|e| format!("Portal unavailable: {}", e))?;

    let mut opts: std::collections::HashMap<String, glib::Variant> =
        std::collections::HashMap::new();
    // Distinguish concurrent requests and avoid collisions in request object path.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let token = format!("shyquiet_{}_{}", std::process::id(), nanos);
    opts.insert("handle_token".to_string(), token.to_variant());
    opts.insert("interactive".to_string(), true.to_variant());

    let params = ("", opts).to_variant();
    let reply = proxy
        .call_future("Screenshot", Some(&params), gio::DBusCallFlags::NONE, -1)
        .await
        .map_err(|e| format!("Portal screenshot request failed: {}", e))?;

    let (handle_path,) = reply
        .get::<(String,)>()
        .ok_or_else(|| "Portal returned invalid request handle".to_string())?;

    let connection = proxy.connection();

    let (response_code, response_data) =
        wait_portal_response(&connection, &handle_path, Duration::from_secs(30)).await?;

    if response_code != 0 {
        return match response_code {
            1 | 2 => Ok(false),
            _ => Err(format!("Portal screenshot failed (code {})", response_code)),
        };
    }

    let uri = response_data
        .get("uri")
        .and_then(|v| v.get::<String>())
        .ok_or_else(|| "Portal response did not include screenshot URI".to_string())?;

    let source_path = gio::File::for_uri(&uri)
        .path()
        .ok_or_else(|| "Portal returned non-local screenshot URI".to_string())?;

    fs::rename(&source_path, output)
        .or_else(|_| {
            fs::copy(&source_path, output).map(|_| ()).map_err(|e| {
                std::io::Error::new(e.kind(), format!("Failed to copy screenshot: {}", e))
            })
        })
        .map_err(|e| format!("Failed to move portal screenshot: {}", e))?;

    let _ = fs::remove_file(source_path);
    Ok(true)
}

async fn wait_portal_response(
    connection: &gio::DBusConnection,
    handle_path: &str,
    timeout: Duration,
) -> Result<PortalResponse, String> {
    let result: SharedPortalResult = std::rc::Rc::new(std::cell::RefCell::new(None));

    let result_c = result.clone();
    let _subscription = connection.subscribe_to_signal(
        Some("org.freedesktop.portal.Desktop"),
        Some("org.freedesktop.portal.Request"),
        Some("Response"),
        Some(handle_path),
        None,
        gio::DBusSignalFlags::NONE,
        move |signal| {
            let params = signal.parameters;
            let parsed = params
                .get::<(u32, std::collections::HashMap<String, glib::Variant>)>()
                .ok_or_else(|| "Portal response payload has unexpected format".to_string());
            *result_c.borrow_mut() = Some(parsed);
        },
    );

    let started = Instant::now();
    loop {
        if let Some(done) = result.borrow_mut().take() {
            return done;
        }

        if started.elapsed() >= timeout {
            return Err("Portal screenshot timed out".to_string());
        }

        glib::timeout_future(Duration::from_millis(50)).await;
    }
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
