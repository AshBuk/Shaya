// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Layer-shell initialization for stealth mode
//!
//! Configures the window as an invisible overlay that:
//! - Is not captured by screen sharing tools (grim, Zoom, Meet)
//! - Does not appear in window lists (Alt+Tab, taskbar)
//! - Floats above other windows

use gtk4::ApplicationWindow;

/// Initialize layer-shell for stealth mode
///
/// This makes the window invisible in screen sharing on Wayland
/// compositors that support wlr-layer-shell (Sway, Hyprland, etc.)
///
/// GNOME does not support this protocol.
#[cfg(target_os = "linux")]
pub fn init(window: &ApplicationWindow) {
    use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

    // Early check: is this even Wayland?
    let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    if session_type != "wayland" {
        tracing::warn!("Not running on Wayland (XDG_SESSION_TYPE={})", session_type);
        tracing::warn!("Stealth mode requires Wayland with wlr-layer-shell support");
        return;
    }

    // Check desktop environment - GNOME doesn't support wlr-layer-shell
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    if desktop.to_lowercase().contains("gnome") {
        tracing::warn!("Running on GNOME - stealth mode unavailable");
        tracing::warn!("GNOME does not support wlr-layer-shell protocol");
        tracing::warn!(
            "Stealth mode works on: Sway, Hyprland, and other wlroots-based compositors"
        );
        tracing::info!("ShyQuiet will run as a normal window (visible in screen share)");
        return;
    }

    // Check if layer-shell is supported
    if !gtk4_layer_shell::is_supported() {
        tracing::warn!("Layer-shell not supported - stealth mode unavailable");
        tracing::warn!(
            "Make sure you're running on Wayland with a compositor that supports wlr-layer-shell"
        );
        return;
    }

    window.init_layer_shell();

    // Layer::Overlay — invisible in screen share
    // This is the highest layer, not captured by screenshot tools
    window.set_layer(Layer::Overlay);

    // KeyboardMode::OnDemand — get keyboard focus only when widgets need it
    window.set_keyboard_mode(KeyboardMode::OnDemand);

    // Exclusive zone -1: don't reserve screen space
    window.set_exclusive_zone(-1);

    // Floating window (no anchors = can be positioned anywhere)
    window.set_anchor(Edge::Left, false);
    window.set_anchor(Edge::Right, false);
    window.set_anchor(Edge::Top, false);
    window.set_anchor(Edge::Bottom, false);

    window.set_namespace("shyquiet-stealth");

    tracing::info!("Stealth mode enabled (layer-shell: Overlay, keyboard=OnDemand)");
}

/// Stub for non-Linux platforms
#[cfg(not(target_os = "linux"))]
pub fn init(_window: &ApplicationWindow) {
    tracing::warn!("Layer-shell not available on this platform");
}

