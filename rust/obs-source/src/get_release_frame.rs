//! Safe core of `libobs/obs-source-get-release-frame.c.h`: the exported
//! async-frame consumer pair `obs_source_get_frame()` and
//! `obs_source_release_frame()`.
//!
//! `get_frame` rejects a missing source with its `obs_source_valid()`
//! diagnostic; otherwise, under `async_mutex`, it takes `cur_async_frame`
//! (leaving the slot empty) and adds one reference to it. `release_frame`
//! ignores a missing frame and destroys the frame of a missing source
//! without locking anything; otherwise, under `async_mutex`, it drops one
//! reference and destroys the frame at zero, or else gives it back to the
//! async cache (`remove_async_frame()`).
//!
//! No intentional differences from C.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

use core::ffi::CStr;

/// The source and frame operations the pair performs, in the C's terms.
pub trait Host {
    /// A live, non-null source.
    type Source: Copy;
    /// A live, non-null frame.
    type Frame: Copy;
    /// `obs_source_valid(NULL, func)`: the `LOG_DEBUG` line.
    fn null_source(&self, func: &'static CStr);
    /// `pthread_mutex_lock(&source->async_mutex)`.
    fn lock(&self, source: Self::Source);
    /// `pthread_mutex_unlock(&source->async_mutex)`.
    fn unlock(&self, source: Self::Source);
    /// Takes `cur_async_frame`, leaving NULL behind.
    fn take_current(&self, source: Self::Source) -> Option<Self::Frame>;
    /// `os_atomic_inc_long(&frame->refs)`.
    fn addref(&self, frame: Self::Frame);
    /// `os_atomic_dec_long(&frame->refs) == 0`.
    fn release_ref(&self, frame: Self::Frame) -> bool;
    /// `obs_source_frame_destroy(frame)`.
    fn destroy(&self, frame: Self::Frame);
    /// `remove_async_frame(source, frame)`.
    fn return_to_cache(&self, source: Self::Source, frame: Self::Frame);
}

/// `obs_source_get_frame(source)`.
pub fn get_frame<H: Host>(host: &H, source: Option<H::Source>) -> Option<H::Frame> {
    let _ = (host, source);
    todo!("obs_source_get_frame safe core (#5757)")
}

/// `obs_source_release_frame(source, frame)`.
pub fn release_frame<H: Host>(host: &H, source: Option<H::Source>, frame: Option<H::Frame>) {
    let _ = (host, source, frame);
    todo!("obs_source_release_frame safe core (#5757)")
}
