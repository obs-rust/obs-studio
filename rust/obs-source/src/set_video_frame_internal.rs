//! Safe core of `libobs/obs-source-set-video-frame-internal.c.h`: the hidden
//! `obs_source_set_video_frame_internal()` (`obs-internal.h`, no `EXPORT`),
//! the graphics-thread worker every `obs_source_set_video_frame*()` entry
//! ends in. It is the sibling of [`crate::preload_video_internal`].
//!
//! A missing source is rejected with the `obs_source_valid()` diagnostic of
//! the public entry, `obs_source_set_video_frame`; a NULL frame returns
//! before the graphics context is entered. There is no `destroying()` check.
//! Everything else runs inside one graphics context, in this order: when
//! there is no preload frame yet or its format, width or height differ from
//! the frame's (`preload_frame_changed()`, the rule of
//! [`crate::frame_copy::preload_changed`]), the old preload frame is
//! destroyed and a new one created at the frame's format and size; the frame
//! is copied into the preload frame; the async textures are sized to and
//! filled from the preload frame and marked fresh; then `last_frame_ts` is
//! set to the frame's timestamp under `async_mutex`.
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

use crate::frame_copy::{Shape, preload_changed};

/// The source, frame and graphics operations it performs, in the C's terms.
pub trait Host {
    /// A live, non-null source.
    type Source: Copy;
    /// A non-null frame the caller keeps alive for the call.
    type Frame: Copy;
    /// `obs_source_valid(NULL, func)`: the `LOG_DEBUG` line.
    fn null_source(&self, func: &'static CStr);
    /// `obs_enter_graphics()`.
    fn enter_graphics(&self);
    /// `source->async_preload_frame`'s shape (`frame_copy_shape()`), or
    /// `None` when it is NULL.
    fn preload_shape(&self, source: Self::Source) -> Option<Shape>;
    /// `frame_copy_shape(frame)`.
    fn frame_shape(&self, frame: Self::Frame) -> Shape;
    /// `obs_source_frame_destroy(source->async_preload_frame)`.
    fn destroy_preload(&self, source: Self::Source);
    /// `source->async_preload_frame = obs_source_frame_create(frame->format,
    /// frame->width, frame->height)`.
    fn create_preload(&self, source: Self::Source, frame: Self::Frame);
    /// `obs_source_frame_copy(source->async_preload_frame, frame)`.
    fn copy_to_preload(&self, source: Self::Source, frame: Self::Frame);
    /// `set_async_texture_size(source, source->async_preload_frame)`.
    fn set_texture_size(&self, source: Self::Source);
    /// `update_async_textures(source, source->async_preload_frame,
    /// source->async_textures, source->async_texrender)`.
    fn update_textures(&self, source: Self::Source);
    /// `os_atomic_set_bool(&source->async_textures_stale, false)`.
    fn mark_textures_fresh(&self, source: Self::Source);
    /// `pthread_mutex_lock(&source->async_mutex)`.
    fn lock_async(&self, source: Self::Source);
    /// `source->last_frame_ts = frame->timestamp`.
    fn set_last_frame_ts(&self, source: Self::Source, frame: Self::Frame);
    /// `pthread_mutex_unlock(&source->async_mutex)`.
    fn unlock_async(&self, source: Self::Source);
    /// `obs_leave_graphics()`.
    fn leave_graphics(&self);
}

/// `obs_source_set_video_frame_internal(source, frame)`.
pub fn set_video_frame_internal<H: Host>(
    host: &H,
    source: Option<H::Source>,
    frame: Option<H::Frame>,
) {
    let Some(source) = source else {
        host.null_source(c"obs_source_set_video_frame");
        return;
    };
    let Some(frame) = frame else {
        return;
    };

    host.enter_graphics();

    let preload = host.preload_shape(source);
    if preload_changed(preload, host.frame_shape(frame)) {
        host.destroy_preload(source);
        host.create_preload(source, frame);
    }

    host.copy_to_preload(source, frame);
    host.set_texture_size(source);
    host.update_textures(source);
    host.mark_textures_fresh(source);

    host.lock_async(source);
    host.set_last_frame_ts(source, frame);
    host.unlock_async(source);

    host.leave_graphics();
}
