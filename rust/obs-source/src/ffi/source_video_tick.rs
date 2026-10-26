//! C ABI for `libobs/obs-source-video-tick.c.h`, the per-frame decisions of
//! `obs_source_video_tick()` and `async_tick()`, declared in
//! `libobs/obs-source-video-tick.h` without `EXPORT`.
//!
//! `obs_source_video_tick()` and `async_tick()` in `obs-source.c.h` (still
//! C) read the source's fields and make the three atomic loads themselves,
//! each at its point, pass the values to these shims and act on what comes
//! back: every call, the filter walks, the stores of `showing` and
//! `active`, the plugin callback and `async_mutex` stay there. So
//! `struct obs_source` is never mirrored, no pointer crosses, and the shims
//! take no lock, call nothing in libobs and touch no memory. All seven stay
//! hidden (`rust-hidden.txt`).
//!
//! The counts are the C's `long` ([`c_long`], 32 bits on Windows), widened
//! to `i64` for the core by `widen()`. The two enums come back as the C's
//! enums do: their values are 0 to 2, so `int` and `unsigned int` agree.
//!
//! No intentional differences from C: every input is a value, so there is
//! no pointer to check.

use std::os::raw::{c_int, c_long};

use crate::video_tick::{self as core, AsyncRoute, Edge};

/// `enum obs_source_video_tick_edge`.
#[allow(non_camel_case_types)]
pub type obs_source_video_tick_edge = u32;
/// `OBS_SOURCE_VIDEO_TICK_EDGE_NONE`.
pub const OBS_SOURCE_VIDEO_TICK_EDGE_NONE: obs_source_video_tick_edge = 0;
/// `OBS_SOURCE_VIDEO_TICK_EDGE_ON`.
pub const OBS_SOURCE_VIDEO_TICK_EDGE_ON: obs_source_video_tick_edge = 1;
/// `OBS_SOURCE_VIDEO_TICK_EDGE_OFF`.
pub const OBS_SOURCE_VIDEO_TICK_EDGE_OFF: obs_source_video_tick_edge = 2;

/// `enum obs_source_video_tick_async_route`.
#[allow(non_camel_case_types)]
pub type obs_source_video_tick_async_route = u32;
/// `OBS_SOURCE_VIDEO_TICK_ASYNC_CLOSEST`.
pub const OBS_SOURCE_VIDEO_TICK_ASYNC_CLOSEST: obs_source_video_tick_async_route = 0;
/// `OBS_SOURCE_VIDEO_TICK_ASYNC_DEINTERLACE`.
pub const OBS_SOURCE_VIDEO_TICK_ASYNC_DEINTERLACE: obs_source_video_tick_async_route = 1;

const fn edge_value(edge: Edge) -> obs_source_video_tick_edge {
    match edge {
        Edge::None => OBS_SOURCE_VIDEO_TICK_EDGE_NONE,
        Edge::On => OBS_SOURCE_VIDEO_TICK_EDGE_ON,
        Edge::Off => OBS_SOURCE_VIDEO_TICK_EDGE_OFF,
    }
}

/// The C's `long` as the core's `i64`: the same type on 64-bit Unix,
/// `i32` on Windows.
#[allow(
    clippy::useless_conversion,
    reason = "c_long is i64 on 64-bit Unix and i32 on Windows"
)]
fn widen(n: c_long) -> i64 {
    i64::from(n)
}

const fn route_value(route: AsyncRoute) -> obs_source_video_tick_async_route {
    match route {
        AsyncRoute::Closest => OBS_SOURCE_VIDEO_TICK_ASYNC_CLOSEST,
        AsyncRoute::Deinterlace => OBS_SOURCE_VIDEO_TICK_ASYNC_DEINTERLACE,
    }
}

/// `bool obs_source_video_tick_transition(int type)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_video_tick_transition(source_type: c_int) -> bool {
    core::transition_tick(source_type)
}

/// `bool obs_source_video_tick_async(uint32_t output_flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_video_tick_async(output_flags: u32) -> bool {
    core::async_tick(output_flags)
}

/// `bool obs_source_video_tick_media(uint32_t output_flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_video_tick_media(output_flags: u32) -> bool {
    core::media_actions(output_flags)
}

/// `bool obs_source_video_tick_deferred_update(long defer_update_count)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_video_tick_deferred_update(defer_update_count: c_long) -> bool {
    core::deferred_update(widen(defer_update_count))
}

/// `enum obs_source_video_tick_edge obs_source_video_tick_edge(long refs,
/// bool was)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_video_tick_edge(
    refs: c_long,
    was: bool,
) -> obs_source_video_tick_edge {
    edge_value(core::edge(widen(refs), was))
}

/// `bool obs_source_video_tick_calls_tick(bool has_data, bool
/// has_callback)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_video_tick_calls_tick(has_data: bool, has_callback: bool) -> bool {
    core::calls_tick(has_data, has_callback)
}

/// `enum obs_source_video_tick_async_route
/// obs_source_video_tick_async_route(bool deinterlacing)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_video_tick_async_route(
    deinterlacing: bool,
) -> obs_source_video_tick_async_route {
    route_value(core::async_route(deinterlacing))
}
