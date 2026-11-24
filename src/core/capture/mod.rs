// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Screenshot capture for Wayland/X11
//!
//! Stealth behavior:
//! - Hyprland: Shaya blacked out with the `no_screen_share` layer rule (see OVERLAY_GUIDE.md)
//! - Sway: Shaya WILL appear in screenshots (no layer filtering support)
//! - Selection UI (slurp overlay): whether a screen share shows it depends on the compositor

use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

type PortalResponse = (u32, std::collections::HashMap<String, glib::Variant>);
type SharedPortalResult = std::rc::Rc<std::cell::RefCell<Option<Result<PortalResponse>>>>;

/// Screenshot result with base64 encoded image
#[derive(Clone)]
pub struct Screenshot {
    pub base64_data: String,
    pub mime_type: String,
}

/// Capture user-selected area (sync path).
/// Wayland wlroots: slurp + grim. KDE: spectacle -r. X11: gnome-screenshot -a.
/// GNOME Wayland uses the async XDG portal instead — see `take_screenshot_async`.
pub fn capture_area() -> Result<Screenshot> {
    if requires_async_portal() {
        bail!(
            "GNOME Wayland area capture requires the async XDG Desktop Portal; \
             use take_screenshot_async() from the UI main loop."
        );
    }

    let output_path = screenshot_path()?;

    tracing::info!("Starting area selection");

    if is_wayland() && has_command("slurp") && has_command("grim") {
        capture_area_slurp_grim(&output_path)?;
    } else if has_command("spectacle") {
        capture_area_spectacle(&output_path)?;
    } else if has_command("gnome-screenshot") {
        capture_area_gnome(&output_path)?;
    } else {
        bail!(
            "No area capture tool. Install: slurp+grim (wlroots), spectacle (KDE), or gnome-screenshot"
        );
    }
    read_and_encode(&output_path)
}

fn screenshot_path() -> Result<PathBuf> {
    crate::core::utils::temp_path("screenshot", "png").context("Failed to prepare screenshot file")
}

/// Take a screenshot of the entire screen
pub fn take_screenshot() -> Result<Screenshot> {
    let output_path = screenshot_path()?;

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
pub async fn take_screenshot_async() -> Result<Option<Screenshot>> {
    if !requires_async_portal() {
        return take_screenshot().map(Some);
    }

    let output_path = screenshot_path()?;

    tracing::info!("Taking GNOME Wayland screenshot via XDG portal");
    if !take_gnome_wayland_screenshot_portal(&output_path).await? {
        return Ok(None);
    }
    read_and_encode(&output_path).map(Some)
}

/// Read screenshot file and encode to base64
fn read_and_encode(path: &Path) -> Result<Screenshot> {
    if !path.exists() {
        bail!("Screenshot file not created");
    }

    let image_data = fs::read(path);
    let _ = fs::remove_file(path);
    let image_data = image_data.context("Failed to read screenshot")?;

    if image_data.len() < 100 {
        bail!("Screenshot too small or cancelled");
    }

    tracing::info!("Screenshot captured: {} bytes", image_data.len());

    Ok(Screenshot {
        base64_data: B64.encode(&image_data),
        mime_type: "image/png".to_string(),
    })
}

/// Capture area using slurp + grim (wlroots compositors)
fn capture_area_slurp_grim(output: &Path) -> Result<()> {
    // Run slurp to get geometry
    let slurp_output = Command::new("slurp")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("Failed to run slurp")?;

    if !slurp_output.status.success() {
        bail!("Area selection cancelled");
    }

    let geometry = String::from_utf8_lossy(&slurp_output.stdout)
        .trim()
        .to_string();

    if geometry.is_empty() {
        bail!("No area selected");
    }
    tracing::info!("Selected area: {}", geometry);

    // Capture with grim
    let status = Command::new("grim")
        .args(["-g", &geometry, output.to_string_lossy().as_ref()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .context("Failed to run grim")?;

    if !status.success() {
        bail!("grim capture failed");
    }
    Ok(())
}

/// Capture area using spectacle -r (KDE Plasma, Wayland and X11).
fn capture_area_spectacle(output: &Path) -> Result<()> {
    let status = Command::new("spectacle")
        .args(["-r", "-b", "-n", "-o", output.to_string_lossy().as_ref()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("Failed to run spectacle")?;

    if !status.success() {
        bail!("Area selection cancelled");
    }
    if output.exists() {
        Ok(())
    } else {
        Err(anyhow!("spectacle succeeded but file was not created"))
    }
}

/// Capture area using gnome-screenshot -a (X11 only).
///
/// On GNOME Wayland, the XDG Desktop Portal async path is used instead
/// (see `take_screenshot_async`), so this function targets X11 only.
fn capture_area_gnome(output: &Path) -> Result<()> {
    let status = Command::new("gnome-screenshot")
        .args(["-a", "-f", output.to_string_lossy().as_ref()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("Failed to run gnome-screenshot")?;

    if !status.success() {
        bail!("Area selection cancelled");
    }

    if output.exists() {
        Ok(())
    } else {
        Err(anyhow!(
            "gnome-screenshot succeeded but file was not created"
        ))
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

fn take_wayland_screenshot(output: &Path) -> Result<()> {
    if is_gnome() {
        bail!(
            "GNOME Wayland screenshots require the async XDG Desktop Portal; \
             use take_screenshot_async() from the UI main loop."
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

    Err(anyhow!(
        "No screenshot tool found. Install: grim (Sway/Hyprland) or spectacle (KDE)"
    ))
}

/// Full screenshot on GNOME Wayland via XDG Desktop Portal.
/// Returns `Ok(true)` on success, `Ok(false)` when user cancelled/denied.
async fn take_gnome_wayland_screenshot_portal(output: &Path) -> Result<bool> {
    let proxy = gio::DBusProxy::for_bus_future(
        gio::BusType::Session,
        gio::DBusProxyFlags::NONE,
        None::<&gio::DBusInterfaceInfo>,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.Screenshot",
    )
    .await
    .context("Portal unavailable")?;

    let mut opts: std::collections::HashMap<String, glib::Variant> =
        std::collections::HashMap::new();
    // Distinguish concurrent requests and avoid collisions in request object path.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let token = format!("shaya_{}_{}", std::process::id(), nanos);
    opts.insert("handle_token".to_string(), token.to_variant());
    opts.insert("interactive".to_string(), true.to_variant());

    // Subscribe before calling: a quick answer can arrive before the call returns.
    let connection = proxy.connection();
    let expected_path = portal_request_path(&connection, &token);
    let mut request = PortalRequest::subscribe(&connection, &expected_path);

    let params = ("", opts).to_variant();
    let reply = proxy
        .call_future("Screenshot", Some(&params), gio::DBusCallFlags::NONE, -1)
        .await
        .context("Portal screenshot request failed")?;

    let (handle_path,) = reply
        .get::<(String,)>()
        .context("Portal returned invalid request handle")?;
    // A portal that ignores handle_token picks its own path.
    if handle_path != expected_path {
        request = PortalRequest::subscribe(&connection, &handle_path);
    }

    // The dialog waits for the user to pick an area, which can take a while.
    let (response_code, response_data) = request.wait(Duration::from_secs(300)).await?;

    if response_code != 0 {
        return match response_code {
            1 | 2 => Ok(false),
            _ => Err(anyhow!("Portal screenshot failed (code {response_code})")),
        };
    }

    let uri = response_data
        .get("uri")
        .and_then(|v| v.get::<String>())
        .context("Portal response did not include screenshot URI")?;

    let source_path = gio::File::for_uri(&uri)
        .path()
        .context("Portal returned non-local screenshot URI")?;

    fs::rename(&source_path, output)
        .or_else(|_| fs::copy(&source_path, output).map(drop))
        .context("Failed to move portal screenshot")?;

    let _ = fs::remove_file(source_path);
    Ok(true)
}

/// Request object path the portal uses for `token`, as the portal spec defines it.
fn portal_request_path(connection: &gio::DBusConnection, token: &str) -> String {
    let sender = connection
        .unique_name()
        .map(|name| name.trim_start_matches(':').replace('.', "_"))
        .unwrap_or_default();
    format!("/org/freedesktop/portal/desktop/request/{sender}/{token}")
}

/// Subscription to the Response signal of one portal request.
struct PortalRequest {
    result: SharedPortalResult,
    _subscription: gio::SignalSubscription,
}

impl PortalRequest {
    fn subscribe(connection: &gio::DBusConnection, handle_path: &str) -> Self {
        let result: SharedPortalResult = std::rc::Rc::new(std::cell::RefCell::new(None));
        let result_c = result.clone();
        let subscription = connection.subscribe_to_signal(
            Some("org.freedesktop.portal.Desktop"),
            Some("org.freedesktop.portal.Request"),
            Some("Response"),
            Some(handle_path),
            None,
            gio::DBusSignalFlags::NONE,
            move |signal| {
                let parsed = signal
                    .parameters
                    .get::<PortalResponse>()
                    .context("Portal response payload has unexpected format");
                *result_c.borrow_mut() = Some(parsed);
            },
        );
        Self {
            result,
            _subscription: subscription,
        }
    }

    async fn wait(self, timeout: Duration) -> Result<PortalResponse> {
        let started = Instant::now();
        loop {
            if let Some(done) = self.result.borrow_mut().take() {
                return done;
            }
            if started.elapsed() >= timeout {
                bail!("Portal screenshot timed out");
            }
            glib::timeout_future(Duration::from_millis(50)).await;
        }
    }
}

fn take_x11_screenshot(output: &Path) -> Result<()> {
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
    Err(anyhow!(
        "No screenshot tool found. Install: gnome-screenshot, scrot, or spectacle"
    ))
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
