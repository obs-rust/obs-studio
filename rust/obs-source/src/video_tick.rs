//! Safe core of `libobs/obs-source-video-tick.c.h`: the per-frame
//! decisions of `obs_source_video_tick()` and `async_tick()` in
//! `obs-source.c`.
//!
//! - [`transition_tick`], [`async_tick`] and [`media_actions`]: which steps
//!   run for the source's type and output flags, each asked at its own
//!   point;
//! - [`deferred_update`]: whether a deferred update is due;
//! - [`edge`]: the show/hide and activate/deactivate edges of a loaded
//!   reference count against the flag it sets;
//! - [`calls_tick`]: whether the plugin's `video_tick` callback is called;
//! - [`async_route`]: whether `async_tick()` deinterlaces the last frame or
//!   takes the closest one, and filters the previous frame too.
//!
//! `obs-source.c` keeps every read and call where it was: the three
//! unlocked `os_atomic_load_long()` reads (the show refs before the show
//! edge runs, the activate refs only after it, since a show callback can
//! change them), `obs_transition_tick()`, `process_media_actions()`,
//! `obs_source_deferred_update()`, `gs_texrender_reset()`, the show, hide,
//! activate and deactivate calls, the filter walks in reverse, the stores
//! of `showing` and `active`, the plugin callback, the per-tick resets,
//! `async_mutex` and both `deinterlacing_enabled()` calls. It passes the
//! values in and acts on what comes back. Nothing here keeps state, takes a
//! lock or calls libobs.

/// `OBS_SOURCE_TYPE_TRANSITION` (`enum obs_source_type`, `obs-source.h`).
pub const TYPE_TRANSITION: i32 = 2;
/// `OBS_SOURCE_ASYNC` (`obs-source.h`).
pub const ASYNC: u32 = 1 << 2;
/// `OBS_SOURCE_CONTROLLABLE_MEDIA` (`obs-source.h`).
pub const CONTROLLABLE_MEDIA: u32 = 1 << 13;

/// `enum obs_source_video_tick_edge`: how a reference count moved the
/// source's `showing` or `active` flag. The discriminants are the C's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Edge {
    /// The flag is the same: nothing is called.
    None = 0,
    /// The count is not 0 and the flag was false: show or activate.
    On = 1,
    /// The count is 0 and the flag was true: hide or deactivate.
    Off = 2,
}

/// `enum obs_source_video_tick_async_route`: what `async_tick()` does with
/// the source's frames. The discriminants are the C's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AsyncRoute {
    /// Drop the current frame and take `get_closest_frame()`.
    Closest = 0,
    /// `deinterlace_process_last_frame()`; the previous frame is filtered
    /// too.
    Deinterlace = 1,
}

/// Whether `obs_transition_tick()` runs: a type of
/// `OBS_SOURCE_TYPE_TRANSITION`.
#[must_use]
pub fn transition_tick(source_type: i32) -> bool {
    let _ = source_type;
    todo!("video_tick::transition_tick")
}

/// Whether `async_tick()` runs: `OBS_SOURCE_ASYNC` in the output flags.
#[must_use]
pub fn async_tick(output_flags: u32) -> bool {
    let _ = output_flags;
    todo!("video_tick::async_tick")
}

/// Whether `process_media_actions()` runs: `OBS_SOURCE_CONTROLLABLE_MEDIA`
/// in the output flags.
#[must_use]
pub fn media_actions(output_flags: u32) -> bool {
    let _ = output_flags;
    todo!("video_tick::media_actions")
}

/// Whether `obs_source_deferred_update()` runs: a loaded
/// `defer_update_count` above 0.
#[must_use]
pub fn deferred_update(defer_update_count: i64) -> bool {
    let _ = defer_update_count;
    todo!("video_tick::deferred_update")
}

/// The edge of a loaded reference count against the flag it sets: the flag
/// is now `refs != 0`; [`Edge::On`] or [`Edge::Off`] when that differs from
/// `was`, [`Edge::None`] when it does not.
#[must_use]
pub fn edge(refs: i64, was: bool) -> Edge {
    let _ = (refs, was);
    todo!("video_tick::edge")
}

/// Whether the plugin's `video_tick` callback is called: the source has
/// data and a callback.
#[must_use]
pub fn calls_tick(has_data: bool, has_callback: bool) -> bool {
    let _ = (has_data, has_callback);
    todo!("video_tick::calls_tick")
}

/// The route of `async_tick()` for `deinterlacing_enabled()`.
#[must_use]
pub fn async_route(deinterlacing: bool) -> AsyncRoute {
    let _ = deinterlacing;
    todo!("video_tick::async_route")
}
