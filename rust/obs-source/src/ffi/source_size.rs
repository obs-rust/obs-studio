//! C ABI for `libobs/obs-source-size.c.h`, the size query routing of
//! `obs_source_get_width()` and `obs_source_get_height()`, declared in
//! `libobs/obs-source-size.h` without `EXPORT`.
//!
//! `get_base_width()` and `get_base_height()` in `obs-source.c.h` (still C)
//! read the source's fields into an [`obs_source_size_node`] passed by value,
//! with the size callback of the axis they ask for, call
//! [`obs_source_size_route`] and follow the route themselves: the
//! transition's actual size, the callback, the recursion into the filter
//! target, the async frame's width or height, or 0. `get_recurse_width()`
//! and `get_recurse_height()` hold the source's `filter_mutex`, call
//! [`obs_source_size_entry`] and start at the first filter or the source.
//! So `struct obs_source` is never mirrored, no source pointer crosses, and
//! these shims take no lock and call nothing in libobs. Both stay hidden
//! (`rust-hidden.txt`).
//!
//! No intentional differences from C.

use std::os::raw::{c_int, c_long};

use crate::size_route::{self as core, Entry, Node, Route};

/// `struct obs_source_size_node` (`obs-source-size.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct obs_source_size_node {
    /// `int type`: `source->info.type`.
    pub source_type: c_int,
    pub has_data: bool,
    pub has_callback: bool,
    pub has_filter_parent: bool,
    pub enabled: bool,
    pub async_active: bool,
    pub async_rotation: c_long,
}

/// `enum obs_source_size_ask`, as the C's enum is returned (an `int` or
/// `unsigned int`; every value fits both).
#[allow(non_camel_case_types)]
pub type obs_source_size_ask = u32;
/// `OBS_SOURCE_SIZE_TRANSITION`.
pub const OBS_SOURCE_SIZE_TRANSITION: obs_source_size_ask = 0;
/// `OBS_SOURCE_SIZE_CALLBACK`.
pub const OBS_SOURCE_SIZE_CALLBACK: obs_source_size_ask = 1;
/// `OBS_SOURCE_SIZE_ASK_TARGET`.
pub const OBS_SOURCE_SIZE_ASK_TARGET: obs_source_size_ask = 2;
/// `OBS_SOURCE_SIZE_ASYNC`.
pub const OBS_SOURCE_SIZE_ASYNC: obs_source_size_ask = 3;
/// `OBS_SOURCE_SIZE_ASYNC_SWAPPED`.
pub const OBS_SOURCE_SIZE_ASYNC_SWAPPED: obs_source_size_ask = 4;
/// `OBS_SOURCE_SIZE_ZERO`.
pub const OBS_SOURCE_SIZE_ZERO: obs_source_size_ask = 5;

/// `enum obs_source_size_start`, returned as the C's enum.
#[allow(non_camel_case_types)]
pub type obs_source_size_start = u32;
/// `OBS_SOURCE_SIZE_ENTRY_SELF`.
pub const OBS_SOURCE_SIZE_ENTRY_SELF: obs_source_size_start = 0;
/// `OBS_SOURCE_SIZE_ENTRY_FIRST_FILTER`.
pub const OBS_SOURCE_SIZE_ENTRY_FIRST_FILTER: obs_source_size_start = 1;

const fn route_value(route: Route) -> obs_source_size_ask {
    match route {
        Route::Transition => OBS_SOURCE_SIZE_TRANSITION,
        Route::Callback => OBS_SOURCE_SIZE_CALLBACK,
        Route::AskTarget => OBS_SOURCE_SIZE_ASK_TARGET,
        Route::Async { swap: false } => OBS_SOURCE_SIZE_ASYNC,
        Route::Async { swap: true } => OBS_SOURCE_SIZE_ASYNC_SWAPPED,
        Route::Zero => OBS_SOURCE_SIZE_ZERO,
    }
}

const fn entry_value(entry: Entry) -> obs_source_size_start {
    match entry {
        Entry::Itself => OBS_SOURCE_SIZE_ENTRY_SELF,
        Entry::FirstFilter => OBS_SOURCE_SIZE_ENTRY_FIRST_FILTER,
    }
}

/// `enum obs_source_size_ask obs_source_size_route(struct
/// obs_source_size_node node)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_size_route(node: obs_source_size_node) -> obs_source_size_ask {
    route_value(core::route(Node {
        source_type: node.source_type,
        has_data: node.has_data,
        has_callback: node.has_callback,
        has_filter_parent: node.has_filter_parent,
        enabled: node.enabled,
        async_active: node.async_active,
        async_rotation: node.async_rotation,
    }))
}

/// `enum obs_source_size_start obs_source_size_entry(int type, bool
/// has_filters)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_size_entry(
    source_type: c_int,
    has_filters: bool,
) -> obs_source_size_start {
    entry_value(core::entry(source_type, has_filters))
}
