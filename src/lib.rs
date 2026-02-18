// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Shaya - Privacy-first AI assistant with stealth mode
//!
//! GTK4 + Layer-Shell for invisible screen sharing on Wayland.

mod core;
mod ui;

pub use ui::stealth;

/// Run the main stealth window
pub fn run() {
    tracing_subscriber::fmt::init();
    tracing::info!("Starting Shaya in STEALTH mode");

    if let Err(e) = stealth::run() {
        tracing::error!("Failed to start stealth UI: {}", e);
        std::process::exit(1);
    }
}
