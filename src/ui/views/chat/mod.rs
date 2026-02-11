// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Chat View - AI interaction with Vision + Audio
//!
//! Split into two parts for input passthrough:
//! - `build_content` - read-only area (passthrough)
//! - `build_input` - interactive controls (receives input)

mod audio;
pub mod markdown;
mod messaging;
mod view;

pub(crate) use view::ChatSharedState;
pub use view::{build_content, build_input, refresh_display};
