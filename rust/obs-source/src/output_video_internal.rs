//! Safe core of `libobs/obs-source-output-video-internal.c.h`: the hidden
//! `obs_source_output_video_internal()` (`obs-internal.h`, no `EXPORT`), the
//! worker every `obs_source_output_video*()` entry ends in. It is the
//! sibling of [`crate::preload_video_internal`] and
//! [`crate::set_video_frame_internal`].
//!
//! A missing source is rejected with the `obs_source_valid()` diagnostic of
//! the public entry, `obs_source_output_video`. Then, under
//! `audio_buf_mutex` alone (never inside `async_mutex`, #3747),
//! `last_output_frame_ts` is set to the frame's timestamp, or 0 for a NULL
//! frame.
//!
//! A NULL frame then, under `async_mutex`, clears `async_active`, marks the
//! textures stale, zeroes `last_frame_ts` and frees the async frame cache.
//!
//! Otherwise the profiler is told a frame arrived and the frame goes through
//! the C `cache_video()`, outside any lock. Then, under `async_mutex`, a
//! cached frame loses the reference `cache_video()` added: at 0 it is
//! destroyed, otherwise it is pushed onto `async_frames` and `async_active`
//! is set. No cached frame (the queue overflowed) touches nothing more.
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

use core::ffi::{CStr, c_long};

/// The source, frame and cache operations it performs, in the C's terms.
pub trait Host {
    /// A live, non-null source.
    type Source: Copy;
    /// A non-null frame the caller keeps alive for the call.
    type Frame: Copy;
    /// A non-null frame of the source's async frame cache.
    type Cached: Copy;
    /// `obs_source_valid(NULL, func)`: the `LOG_DEBUG` line.
    fn null_source(&self, func: &'static CStr);
    /// `pthread_mutex_lock(&source->audio_buf_mutex)`.
    fn lock_audio_buf(&self, source: Self::Source);
    /// `frame->timestamp`.
    fn frame_timestamp(&self, frame: Self::Frame) -> u64;
    /// `source->last_output_frame_ts = timestamp`.
    fn set_last_output_frame_ts(&self, source: Self::Source, timestamp: u64);
    /// `pthread_mutex_unlock(&source->audio_buf_mutex)`.
    fn unlock_audio_buf(&self, source: Self::Source);
    /// `pthread_mutex_lock(&source->async_mutex)`.
    fn lock_async(&self, source: Self::Source);
    /// `os_atomic_set_bool(&source->async_active, active)`.
    fn set_async_active(&self, source: Self::Source, active: bool);
    /// `os_atomic_set_bool(&source->async_textures_stale, true)`.
    fn mark_textures_stale(&self, source: Self::Source);
    /// `source->last_frame_ts = 0`.
    fn clear_last_frame_ts(&self, source: Self::Source);
    /// `free_async_cache(source)`.
    fn free_async_cache(&self, source: Self::Source);
    /// `pthread_mutex_unlock(&source->async_mutex)`.
    fn unlock_async(&self, source: Self::Source);
    /// `source_profiler_async_frame_received(source)`.
    fn frame_received(&self, source: Self::Source);
    /// `cache_video(source, frame)`: the cached copy with one reference
    /// added, or `None` when the queue overflowed.
    fn cache_video(&self, source: Self::Source, frame: Self::Frame) -> Option<Self::Cached>;
    /// `os_atomic_dec_long(&cached->refs)`: the count left.
    fn release_ref(&self, cached: Self::Cached) -> c_long;
    /// `obs_source_frame_destroy(cached)`.
    fn destroy_frame(&self, cached: Self::Cached);
    /// `da_push_back(source->async_frames, &cached)`.
    fn push_frame(&self, source: Self::Source, cached: Self::Cached);
}

/// `obs_source_output_video_internal(source, frame)`.
pub fn output_video_internal<H: Host>(
    host: &H,
    source: Option<H::Source>,
    frame: Option<H::Frame>,
) {
    let _ = (host, source, frame);
    todo!("obs_source_output_video_internal")
}
