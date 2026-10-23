//! Safe core of `libobs/obs-source-color-space.c.h`: the colour space query
//! routing of `obs_source_get_color_space()`.
//!
//! A source asked for its colour space either answers or hands the
//! question on. [`route`] decides which, from the fields of the source in a
//! [`Node`]:
//! - a non-filter source with no video asks its filter parent;
//! - a filter with no video asks its target (#1493: an audio filter must
//!   not answer for the chain below it, or HDR Tone Mapping sees SDR);
//! - a source with no data, or a disabled one, asks its target;
//! - an async source picks from the preferred spaces ([`pick`]);
//! - any other source asks its `video_get_color_space` callback.
//!
//! [`pick`] is the async pick: the first preferred space equal to the async
//! frame's space, else the last preferred space, else (none) the frame's
//! space.
//!
//! `obs-source.c.h` keeps `data_valid()`, reads the fields, follows the
//! route (the recursion into the parent or the target,
//! `convert_video_space()` of the frame's format and transfer, and the
//! callback, or `GS_CS_SRGB` without one) and returns what comes back.
//! Nothing here keeps state, takes a lock, calls libobs or follows a source
//! pointer.
//!
//! Colour spaces are `enum gs_color_space` values as `i32`, compared and
//! returned as they are: a value outside the enum is passed through, as in
//! the C. Characterized, not endorsed (`test_source_color_space_steps`), and
//! kept: a filter flagged async picks unless the earlier rules send it to
//! its target; with no data and nowhere to go the route is the callback or
//! the pick.

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

/// `OBS_SOURCE_TYPE_FILTER` (`enum obs_source_type`).
pub const TYPE_FILTER: i32 = 1;

/// `OBS_SOURCE_VIDEO` (`obs-source.h`).
pub const VIDEO: u32 = 1 << 0;

/// `OBS_SOURCE_ASYNC` (`obs-source.h`).
pub const ASYNC: u32 = 1 << 2;

/// `struct obs_source_color_space_node`: the fields of the source the route
/// reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Node {
    /// `source->info.type`, an `enum obs_source_type`.
    pub source_type: i32,
    /// `source->info.output_flags`.
    pub output_flags: u32,
    /// `source->context.data != NULL`.
    pub has_data: bool,
    /// `source->enabled`.
    pub enabled: bool,
    /// `source->filter_parent != NULL`.
    pub has_filter_parent: bool,
    /// `source->filter_target != NULL`.
    pub has_filter_target: bool,
}

/// `enum obs_source_color_space_ask`: which source
/// `obs_source_get_color_space()` asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Route {
    /// `OBS_SOURCE_COLOR_SPACE_ASK_PARENT`: the filter parent.
    AskParent,
    /// `OBS_SOURCE_COLOR_SPACE_ASK_TARGET`: the filter target.
    AskTarget,
    /// `OBS_SOURCE_COLOR_SPACE_ASYNC_PICK`: [`pick`] against the async
    /// frame's space.
    AsyncPick,
    /// `OBS_SOURCE_COLOR_SPACE_CALLBACK`: `info.video_get_color_space`, or
    /// `GS_CS_SRGB` without one.
    Callback,
}

/// `obs_source_color_space_route()`: which source answers for `node`. Only
/// [`TYPE_FILTER`] is a filter, and only the [`VIDEO`] and [`ASYNC`] bits of
/// the flags count. The rules are tried in the C's order.
pub fn route(node: Node) -> Route {
    let is_filter = node.source_type == TYPE_FILTER;
    let has_video = node.output_flags & VIDEO != 0;

    if !is_filter && !has_video && node.has_filter_parent {
        return Route::AskParent;
    }

    // A filter that produces no video must not answer for the chain below
    // it (#1493).
    if is_filter && !has_video && node.has_filter_target {
        return Route::AskTarget;
    }

    if (!node.has_data || !node.enabled) && node.has_filter_target {
        return Route::AskTarget;
    }

    if node.output_flags & ASYNC != 0 {
        return Route::AsyncPick;
    }

    Route::Callback
}

/// `obs_source_color_space_pick()`: the async pick of `video_space`, the
/// async frame's space, from `preferred`: the first entry equal to it, else
/// the last entry, else `video_space`.
pub fn pick(video_space: i32, preferred: &[i32]) -> i32 {
    let mut space = video_space;
    for &preferred_space in preferred {
        space = preferred_space;
        if space == video_space {
            break;
        }
    }
    space
}
