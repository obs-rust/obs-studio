//! C ABI for `libobs/obs-source-color-space.c.h`, the colour space query
//! routing of `obs_source_get_color_space()`, declared in
//! `libobs/obs-source-color-space.h` without `EXPORT`.
//!
//! `obs_source_get_color_space()` in `obs-source.c.h` (still C) checks
//! `data_valid()`, reads the source's fields into a
//! [`obs_source_color_space_node`] passed by value, calls
//! [`obs_source_color_space_route`] and follows the route itself: the
//! recursion into the filter parent or target, `convert_video_space()` and
//! [`obs_source_color_space_pick`] for an async source, or the callback. So
//! `struct obs_source` is never mirrored, no source pointer crosses, and
//! these shims take no lock and call nothing in libobs. Both stay hidden
//! (`rust-hidden.txt`).
//!
//! Intentional difference from C: a null `preferred_spaces` with a count
//! above 0 is read as an empty list, so the pick gives the frame's space
//! (the C reads through the null pointer). libobs passes the caller's list
//! with its count, and a count of 0 with `NULL`, which both read nothing.

use std::os::raw::c_int;

use crate::color_space::{self as core, Node, Route};

/// `struct obs_source_color_space_node` (`obs-source-color-space.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct obs_source_color_space_node {
    /// `int type`: `source->info.type`.
    pub source_type: c_int,
    pub output_flags: u32,
    pub has_data: bool,
    pub enabled: bool,
    pub has_filter_parent: bool,
    pub has_filter_target: bool,
}

/// `enum obs_source_color_space_ask`, as the C's enum is returned (an
/// `int` or `unsigned int`; every value fits both).
#[allow(non_camel_case_types)]
pub type obs_source_color_space_ask = u32;
/// `OBS_SOURCE_COLOR_SPACE_ASK_PARENT`.
pub const OBS_SOURCE_COLOR_SPACE_ASK_PARENT: obs_source_color_space_ask = 0;
/// `OBS_SOURCE_COLOR_SPACE_ASK_TARGET`.
pub const OBS_SOURCE_COLOR_SPACE_ASK_TARGET: obs_source_color_space_ask = 1;
/// `OBS_SOURCE_COLOR_SPACE_ASYNC_PICK`.
pub const OBS_SOURCE_COLOR_SPACE_ASYNC_PICK: obs_source_color_space_ask = 2;
/// `OBS_SOURCE_COLOR_SPACE_CALLBACK`.
pub const OBS_SOURCE_COLOR_SPACE_CALLBACK: obs_source_color_space_ask = 3;

const fn route_value(route: Route) -> obs_source_color_space_ask {
    match route {
        Route::AskParent => OBS_SOURCE_COLOR_SPACE_ASK_PARENT,
        Route::AskTarget => OBS_SOURCE_COLOR_SPACE_ASK_TARGET,
        Route::AsyncPick => OBS_SOURCE_COLOR_SPACE_ASYNC_PICK,
        Route::Callback => OBS_SOURCE_COLOR_SPACE_CALLBACK,
    }
}

/// `enum obs_source_color_space_ask obs_source_color_space_route(struct
/// obs_source_color_space_node node)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_color_space_route(
    node: obs_source_color_space_node,
) -> obs_source_color_space_ask {
    route_value(core::route(Node {
        source_type: node.source_type,
        output_flags: node.output_flags,
        has_data: node.has_data,
        enabled: node.enabled,
        has_filter_parent: node.has_filter_parent,
        has_filter_target: node.has_filter_target,
    }))
}

/// `int obs_source_color_space_pick(int video_space, size_t count, const
/// int *preferred_spaces)`.
///
/// # Safety
///
/// When `count` is above 0, `preferred_spaces` is null or points to
/// `count` initialized, aligned `int`s that nothing writes during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_color_space_pick(
    video_space: c_int,
    count: usize,
    preferred_spaces: *const c_int,
) -> c_int {
    let preferred: &[c_int] = if count == 0 || preferred_spaces.is_null() {
        &[]
    } else {
        // SAFETY: non-null, with count readable ints (caller contract).
        unsafe { std::slice::from_raw_parts(preferred_spaces, count) }
    };
    core::pick(video_space, preferred)
}
