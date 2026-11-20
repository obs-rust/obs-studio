//! Safe core of `libobs/obs-output-get-name.c.h`: `obs_output_get_name()`
//! borrows the output's current context name.
//!
//! A missing output emits `obs_output_valid()`'s diagnostic and reads
//! nothing. A present output's name is projected exactly once and returned
//! unchanged, NULL, empty and non-UTF-8 names included: no copy, cache,
//! reference or lock.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

/// `None` is the NULL output: `log_null` runs once and nothing is read.
/// Otherwise `project` reads the live output's name once.
pub fn get_name<T>(output: Option<impl FnOnce() -> T>, log_null: impl FnOnce()) -> Option<T> {
    let Some(project) = output else {
        log_null();
        return None;
    };
    Some(project())
}
