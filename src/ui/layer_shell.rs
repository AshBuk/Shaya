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
//! See docs/OVERLAY_GUIDE.md for compositor configuration.

use crate::core::state::Settings;
use gtk4::ApplicationWindow;
use std::cell::Cell;

/// Initialize layer-shell for stealth mode
///
/// Layer-shell alone does not hide the window from capture. Whether it appears in
/// screenshots and screen sharing depends on the compositor, see docs/OVERLAY_GUIDE.md.
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

    // Restore saved position; fit pulls it back on screen and remembers it
    window.set_margin(Edge::Left, settings.window_x);
    window.set_margin(Edge::Top, settings.window_y);
    fit(window, Visible::Window);

    window.set_namespace(Some("shaya-stealth"));

    tracing::info!("Stealth mode enabled (layer-shell: Overlay, keyboard=OnDemand)");
}

/// Part of the window that has to stay on screen
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Visible {
    /// The whole window (the panel)
    Window,
    /// A square of this side, centred in the window (the orb and its ring)
    Centered(i32),
}

type Point = (i32, i32);

thread_local! {
    static VISIBLE: Cell<Visible> = const { Cell::new(Visible::Window) };
    /// Where the user left the window, and where `fit` moved it from there
    static HOME: Cell<Option<(Point, Point)>> = const { Cell::new(None) };
}

/// Switch the part that has to stay visible and move the window to fit it.
///
/// Growing near an edge (orb to ring, ring to panel) nudges the window in;
/// shrinking back returns it where the user left it, unless it was dragged
/// in between.
#[cfg(target_os = "linux")]
pub fn fit(window: &ApplicationWindow, visible: Visible) {
    use gtk4_layer_shell::{Edge, LayerShell};

    if !window.is_layer_window() {
        return;
    }
    VISIBLE.with(|v| v.set(visible));
    let current = (window.margin(Edge::Left), window.margin(Edge::Top));
    let home = match HOME.take() {
        Some((home, moved)) if moved == current => home,
        _ => current,
    };
    let placed = place(window, home.0, home.1);
    if placed != home {
        HOME.set(Some((home, placed)));
    }
}

/// Move a layer-shell window to (x, y), keeping its visible part on its
/// monitor. Returns the applied position.
#[cfg(target_os = "linux")]
pub fn place(window: &ApplicationWindow, x: i32, y: i32) -> (i32, i32) {
    use gtk4::prelude::*;
    use gtk4_layer_shell::{Edge, LayerShell};

    let (x, y) = match monitor_of(window) {
        Some(monitor) => {
            let area = monitor.geometry();
            // Before the first allocation the size is still zero
            let (w, h) = match (window.width(), window.height()) {
                (0, _) | (_, 0) => window.default_size(),
                size => size,
            };
            let visible = VISIBLE.with(Cell::get);
            (
                clamp(x, w, visible, area.width()),
                clamp(y, h, visible, area.height()),
            )
        }
        None => (x, y),
    };
    window.set_margin(Edge::Left, x);
    window.set_margin(Edge::Top, y);
    (x, y)
}

/// The monitor the window is on, or the one it will open on
#[cfg(target_os = "linux")]
fn monitor_of(window: &ApplicationWindow) -> Option<gtk4::gdk::Monitor> {
    use gtk4::prelude::*;
    use gtk4_layer_shell::LayerShell;

    let display = WidgetExt::display(window);
    window
        .surface()
        .and_then(|surface| display.monitor_at_surface(&surface))
        .or_else(|| window.monitor())
        .or_else(|| display.monitors().item(0).and_downcast())
}

/// Clamp a margin on one axis. Margins are offsets from the output's corner
/// (exclusive zone -1 ignores panels). When the visible part is larger than
/// the monitor, its start edge wins.
fn clamp(pos: i32, window: i32, visible: Visible, monitor: i32) -> i32 {
    let (offset, len) = match visible {
        Visible::Window => (0, window),
        Visible::Centered(side) => ((window - side) / 2, side),
    };
    let min = -offset;
    pos.clamp(min, (monitor - offset - len).max(min))
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

#[cfg(not(target_os = "linux"))]
pub fn fit(_window: &ApplicationWindow, _visible: Visible) {}

#[cfg(test)]
mod tests {
    use super::{clamp, Visible};

    const MONITOR: i32 = 1080;
    const WINDOW: i32 = 741;
    const ORB: Visible = Visible::Centered(88);

    #[test]
    fn keeps_on_screen_position() {
        assert_eq!(clamp(100, WINDOW, Visible::Window, MONITOR), 100);
    }

    #[test]
    fn window_stops_at_both_edges() {
        assert_eq!(clamp(-229, WINDOW, Visible::Window, MONITOR), 0);
        assert_eq!(clamp(1000, WINDOW, Visible::Window, MONITOR), 339);
    }

    #[test]
    fn oversized_window_pins_to_start() {
        assert_eq!(clamp(10, 1200, Visible::Window, MONITOR), 0);
    }

    #[test]
    fn orb_reaches_the_corner() {
        // The orb's edge sits (741 - 88) / 2 = 326 px inside the window
        assert_eq!(clamp(-1000, WINDOW, ORB, MONITOR), -326);
        assert_eq!(clamp(1000, WINDOW, ORB, MONITOR), MONITOR - 326 - 88);
    }

    #[test]
    fn orb_moves_freely_on_screen() {
        assert_eq!(clamp(-229, WINDOW, ORB, MONITOR), -229);
    }
}
