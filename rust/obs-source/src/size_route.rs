//! Safe core of `libobs/obs-source-size.c.h`: the size query routing of
//! `obs_source_get_width()` and `obs_source_get_height()`.
//!
//! A source asked for its size either answers or hands the question on.
//! [`route`] decides how, from the fields of the source in a [`Node`]
//! (`get_base_width()` and `get_base_height()`):
//! - a transition reports its actual size while enabled, and 0 otherwise;
//! - a source with data and a size callback asks it (a filter only while
//!   enabled: a disabled filter does not resize);
//! - a filter without one asks its target;
//! - an active async source reports its frame's size, the axes swapped
//!   unless the rotation is a multiple of 180 ([`swaps`]);
//! - anything else reports 0.
//!
//! A filter is a source with a filter parent, not one of the filter type.
//! [`entry`] says where `obs_source_get_width()` starts: a filter at itself,
//! any other source at its first filter if it has filters, else at itself.
//!
//! One route serves both axes: `obs-source.c.h` reads the size callback of
//! the axis it asks for into the node and applies the route to the width or
//! the height. It keeps `data_valid()`, `filter_mutex`, the recursion into
//! the target and the first filter, the `get_width` and `get_height`
//! callbacks and the field each route reads. Nothing here keeps state, takes
//! a lock, calls libobs or follows a source pointer.
//!
//! The rotation is the C's `long` ([`c_long`]: 64 bits on Linux and macOS,
//! 32 on Windows), tested with the C's truncating `%`. Characterized, not
//! endorsed (`test_source_size_route_steps`), and kept: a filter with no
//! parent asks its callback even while disabled, and without one reports its
//! async frame or 0; a source with a callback but no data reports its async
//! frame or 0.

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

use core::ffi::c_long;

/// `OBS_SOURCE_TYPE_FILTER` (`enum obs_source_type`).
pub const TYPE_FILTER: i32 = 1;

/// `OBS_SOURCE_TYPE_TRANSITION` (`enum obs_source_type`).
pub const TYPE_TRANSITION: i32 = 2;

/// `struct obs_source_size_node`: the fields of the source the route reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Node {
    /// `source->info.type`, an `enum obs_source_type`.
    pub source_type: i32,
    /// `source->context.data != NULL`.
    pub has_data: bool,
    /// `source->info.get_width != NULL`, or `get_height` for the height.
    pub has_callback: bool,
    /// `source->filter_parent != NULL`: the source is a filter in a chain.
    pub has_filter_parent: bool,
    /// `source->enabled`.
    pub enabled: bool,
    /// `source->async_active`.
    pub async_active: bool,
    /// `source->async_rotation`.
    pub async_rotation: c_long,
}

/// `enum obs_source_size_ask`: where the size comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Route {
    /// `OBS_SOURCE_SIZE_TRANSITION`: `transition_actual_cx` or `_cy`.
    Transition,
    /// `OBS_SOURCE_SIZE_CALLBACK`: `info.get_width` or `info.get_height`.
    Callback,
    /// `OBS_SOURCE_SIZE_ASK_TARGET`: the filter target's base size.
    AskTarget,
    /// `OBS_SOURCE_SIZE_ASYNC` (`swap` false: `async_width` for the width)
    /// or `OBS_SOURCE_SIZE_ASYNC_SWAPPED` (`swap` true: `async_height` for
    /// the width).
    Async {
        /// The rotation is not a multiple of 180.
        swap: bool,
    },
    /// `OBS_SOURCE_SIZE_ZERO`: 0.
    Zero,
}

/// `enum obs_source_size_start`: where `obs_source_get_width()` starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Entry {
    /// `OBS_SOURCE_SIZE_ENTRY_SELF`: the source itself.
    Itself,
    /// `OBS_SOURCE_SIZE_ENTRY_FIRST_FILTER`: `source->filters.array[0]`.
    FirstFilter,
}

/// `(rotation % 180) != 0`, the C's test on its `long`: whether an async
/// frame's axes swap.
///
/// `wrapping_rem` is `%` for every rotation: it wraps only for a divisor of
/// -1. Its sign follows the rotation, as C99's `%` does, but only whether it
/// is 0 counts, and that is the same for truncating and flooring division.
pub const fn swaps(rotation: c_long) -> bool {
    rotation.wrapping_rem(180) != 0
}

/// `obs_source_size_route()`: where the size of the source in `node` comes
/// from. The rules are tried in the C's order.
pub const fn route(node: Node) -> Route {
    let is_filter = node.has_filter_parent;
    let func_valid = node.has_data && node.has_callback;

    if node.source_type == TYPE_TRANSITION {
        return if node.enabled {
            Route::Transition
        } else {
            Route::Zero
        };
    }

    // A disabled filter does not resize; a source that is not a filter
    // answers enabled or not.
    if func_valid && (!is_filter || node.enabled) {
        return Route::Callback;
    }

    if is_filter {
        return Route::AskTarget;
    }

    if !node.async_active {
        return Route::Zero;
    }

    Route::Async {
        swap: swaps(node.async_rotation),
    }
}

/// `obs_source_size_entry()`: where `obs_source_get_width()` starts, from
/// `source->info.type` and `source->filters.num != 0`.
pub const fn entry(source_type: i32, has_filters: bool) -> Entry {
    if source_type == TYPE_FILTER {
        return Entry::Itself;
    }

    if has_filters {
        Entry::FirstFilter
    } else {
        Entry::Itself
    }
}
