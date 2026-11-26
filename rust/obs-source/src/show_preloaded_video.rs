//! Safe core of `libobs/obs-source-show-preloaded-video.c.h`: the exported
//! `obs_source_show_preloaded_video()`, which puts a source's preloaded
//! async frame on screen.
//!
//! A missing source is rejected with its `obs_source_valid()` diagnostic; a
//! source being destroyed, or one without a preload frame, is left alone.
//! Otherwise, inside one graphics context, the preload frame sizes and fills
//! the source's async textures, which are then marked fresh and the source
//! active. After the context is left, `last_frame_ts` is read under
//! `async_mutex`, and under `audio_buf_mutex` the audio timing and buffer
//! restart at the current time (time zero for a monitor-only source).
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

/// The source, graphics, clock and audio operations it performs, in the C's
/// terms.
pub trait Host {
    /// A live, non-null source.
    type Source: Copy;
    /// `obs_source_valid(NULL, func)`: the `LOG_DEBUG` line.
    fn null_source(&self, func: &'static CStr);
    /// `destroying(source)`.
    fn destroying(&self, source: Self::Source) -> bool;
    /// `source->async_preload_frame != NULL`.
    fn has_preload_frame(&self, source: Self::Source) -> bool;
    /// `obs_enter_graphics()`.
    fn enter_graphics(&self);
    /// `set_async_texture_size(source, source->async_preload_frame)`.
    fn set_texture_size(&self, source: Self::Source);
    /// `update_async_textures(source, source->async_preload_frame,
    /// source->async_textures, source->async_texrender)`.
    fn update_textures(&self, source: Self::Source);
    /// `os_atomic_set_bool(&source->async_textures_stale, false)`.
    fn mark_textures_fresh(&self, source: Self::Source);
    /// `os_atomic_set_bool(&source->async_active, true)`.
    fn mark_active(&self, source: Self::Source);
    /// `obs_leave_graphics()`.
    fn leave_graphics(&self);
    /// `pthread_mutex_lock(&source->async_mutex)`.
    fn lock_async(&self, source: Self::Source);
    /// `source->last_frame_ts`.
    fn last_frame_ts(&self, source: Self::Source) -> u64;
    /// `pthread_mutex_unlock(&source->async_mutex)`.
    fn unlock_async(&self, source: Self::Source);
    /// `pthread_mutex_lock(&source->audio_buf_mutex)`.
    fn lock_audio(&self, source: Self::Source);
    /// `source->monitoring_type == OBS_MONITORING_TYPE_MONITOR_ONLY`.
    fn monitor_only(&self, source: Self::Source) -> bool;
    /// `os_gettime_ns()`.
    fn now(&self) -> u64;
    /// `reset_audio_timing(source, timestamp, os_time)`.
    fn reset_audio_timing(&self, source: Self::Source, timestamp: u64, os_time: u64);
    /// `reset_audio_data_at_sys_time(source, os_time)`.
    fn reset_audio_data(&self, source: Self::Source, os_time: u64);
    /// `pthread_mutex_unlock(&source->audio_buf_mutex)`.
    fn unlock_audio(&self, source: Self::Source);
}

/// `obs_source_show_preloaded_video(source)`.
pub fn show_preloaded_video<H: Host>(host: &H, source: Option<H::Source>) {
    let _ = (host, source);
    todo!()
}
