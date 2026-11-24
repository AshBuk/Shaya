// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Chat View - AI interaction with Vision + Audio
//!
//! Split into two parts:
//! - `build_content` - transcript and status, a page of the view stack
//! - `build_input` - composer and controls, in the panel below the stack

mod audio;
mod live;
pub mod markdown;
mod messaging;
mod status;
mod task;
mod transcript;
mod view;

pub(crate) use transcript::Transcript;
pub(crate) use view::ChatSharedState;
pub use view::{build_content, build_input};
