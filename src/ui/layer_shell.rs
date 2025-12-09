// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Layer-shell initialization for stealth mode
//!
//! Configures the window as an invisible overlay that:
//! - Is NOT captured by wlr-screencopy tools (grim, slurp)
//! - Does not appear in window lists (Alt+Tab, taskbar)
//! - Floats above other windows
//!
//! IMPORTANT: PipeWire-based capture (Zoom, Meet, OBS) WILL capture this window.
//! For full stealth during screen share, use virtual monitor setup.
//! See docs/STEALTH_GUIDE.md for details.
//!
//! Fallback for GNOME/X11: normal window (share specific window for pseudo-stealth)

use gtk4::ApplicationWindow;

/// Initialize layer-shell for stealth mode
///
/// Makes window invisible to wlr-screencopy protocol (grim, slurp).
#[cfg(target_os = "linux")]
pub fn init(window: &ApplicationWindow) {
    use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

    // Early check: is this even Wayland?
    let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    if session_type != "wayland" {
        tracing::warn!("Not running on Wayland (XDG_SESSION_TYPE={})", session_type);
        tracing::warn!("Stealth mode requires Wayland with wlr-layer-shell support");
        apply_fallback(window);
        return;
    }

    // Check desktop environment - GNOME doesn't support wlr-layer-shell
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    if desktop.to_lowercase().contains("gnome") {
        tracing::warn!("Running on GNOME - layer-shell unavailable");
        tracing::info!("Tip: Share a specific window (not entire screen) for pseudo-stealth");
        apply_fallback(window);
        return;
    }

    // Check if layer-shell is supported
    if !gtk4_layer_shell::is_supported() {
        tracing::warn!("Layer-shell not supported by compositor");
        apply_fallback(window);
        return;
    }

    window.init_layer_shell();

    // Layer::Overlay — not captured by wlr-screencopy (grim)
    // NOTE: PipeWire-based capture (Zoom/Meet) WILL see this layer
    window.set_layer(Layer::Overlay);

    // KeyboardMode::None — HUD mode: no auto-focus, type freely in windows below
    // Click inside the window to get keyboard focus when needed
    window.set_keyboard_mode(KeyboardMode::None);

    // Exclusive zone -1: don't reserve screen space
    window.set_exclusive_zone(-1);

    // Floating window (no anchors = can be positioned anywhere)
    window.set_anchor(Edge::Left, false);
    window.set_anchor(Edge::Right, false);
    window.set_anchor(Edge::Top, false);
    window.set_anchor(Edge::Bottom, false);

    window.set_namespace("shyquiet-stealth");

    tracing::info!("Stealth mode enabled (layer-shell: Overlay, keyboard=None)");
}

/// Fallback for environments without layer-shell (GNOME, X11)
///
/// On GNOME/X11 we cannot make window always-on-top programmatically.
/// Window behaves as normal GTK4 window with manual control.
/// Pseudo-stealth: share a specific window (not entire screen) to hide overlay.
#[cfg(target_os = "linux")]
fn apply_fallback(_window: &ApplicationWindow) {
    tracing::info!("Fallback mode: normal window (GNOME/X11)");
    tracing::info!("Tip: Share a specific window (not entire screen) for pseudo-stealth");
}

/// Stub for non-Linux platforms
#[cfg(not(target_os = "linux"))]
pub fn init(_window: &ApplicationWindow) {
    tracing::warn!("Layer-shell not available on this platform");
}
