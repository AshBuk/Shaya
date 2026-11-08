// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Layer-shell initialization for stealth mode.
//!
//! Uses wlr-layer-shell protocol with namespace "shaya-stealth".
//! Compositor rules can target this namespace for stealth behavior.
//!
//! See docs/STEALTH_GUIDE.md for compositor configuration.

use crate::core::state::Settings;
use gtk4::ApplicationWindow;

/// Initialize layer-shell for stealth mode
///
/// Layer-shell alone does not hide the window from capture. Whether it appears in
/// screenshots and screen sharing depends on the compositor, see docs/STEALTH_GUIDE.md.
#[cfg(target_os = "linux")]
pub fn init(window: &ApplicationWindow, settings: &Settings) {
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

    // Overlay layer: floats above all windows, hidden from Alt+Tab
    window.set_layer(Layer::Overlay);

    // KeyboardMode::OnDemand — get keyboard focus when clicked
    window.set_keyboard_mode(KeyboardMode::OnDemand);

    // Exclusive zone -1: don't reserve screen space
    window.set_exclusive_zone(-1);

    // Anchor to top-left for margin-based positioning (drag support)
    // Margins control position: margin_left = X, margin_top = Y
    window.set_anchor(Edge::Left, true);
    window.set_anchor(Edge::Top, true);
    window.set_anchor(Edge::Right, false);
    window.set_anchor(Edge::Bottom, false);

    // Restore saved position
    window.set_margin(Edge::Left, settings.window_x);
    window.set_margin(Edge::Top, settings.window_y);

    window.set_namespace(Some("shaya-stealth"));

    tracing::info!("Stealth mode enabled (layer-shell: Overlay, keyboard=OnDemand)");
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
pub fn init(_window: &ApplicationWindow, _settings: &Settings) {
    tracing::warn!("Layer-shell not available on this platform");
}
