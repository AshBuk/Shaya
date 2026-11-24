// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Background work started from the chat.

use crate::core::state::AppState;
use anyhow::{anyhow, Result};
use gtk4::gio;
use std::cell::RefCell;
use std::rc::Rc;

/// The chat a background action belongs to. Clearing the chat or opening another
/// conversation ends it.
#[derive(Clone, Copy)]
pub(super) struct ChatScope(u64);

impl ChatScope {
    pub fn current(state: &AppState) -> Self {
        Self(state.chat_generation)
    }

    pub fn is_open(self, state: &AppState) -> bool {
        state.chat_generation == self.0
    }
}

/// Run `work` on a worker thread for the chat in `scope`. Returns `None` when that
/// chat is gone, before the work starts or after it finishes, so a result never
/// lands in another chat.
pub(super) async fn run<T: Send + 'static>(
    state: &Rc<RefCell<AppState>>,
    scope: ChatScope,
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Option<Result<T>> {
    if !scope.is_open(&state.borrow()) {
        return None;
    }
    let result = gio::spawn_blocking(work).await.unwrap_or_else(|e| {
        tracing::error!("Chat task panicked: {e:?}");
        Err(anyhow!("Background task failed"))
    });
    scope.is_open(&state.borrow()).then_some(result)
}
