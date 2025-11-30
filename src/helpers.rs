// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Common helper functions for ShyQuiet
//!
//! Provides utilities for safe mutex handling and other common operations.

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Safely lock a mutex, recovering from poison if necessary.
///
/// This helper handles the common pattern of locking a mutex while
/// gracefully recovering from a poisoned state (when a thread panicked
/// while holding the lock).
///
/// # Examples
///
/// ```
/// use std::sync::Mutex;
/// use crate::helpers::lock_or_recover;
///
/// let mutex = Mutex::new(42);
/// let guard = lock_or_recover(&mutex);
/// assert_eq!(*guard, 42);
/// ```
pub fn lock_or_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            tracing::warn!("Mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    }
}

/// Try to lock a mutex with recovery, returning Result for error handling.
///
/// Use this when you need to handle the lock failure explicitly.
pub fn try_lock_or_recover<T>(
    mutex: &Mutex<T>,
) -> Result<MutexGuard<'_, T>, PoisonError<MutexGuard<'_, T>>> {
    mutex.lock()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_or_recover_normal() {
        let mutex = Mutex::new(42);
        let guard = lock_or_recover(&mutex);
        assert_eq!(*guard, 42);
    }
}
