//! Safe core of `libobs/obs-source-filter-chain.c.h`: the filter chain
//! decisions of `obs-source.c.h`.
//!
//! A source's filters are a list: index 0 renders the source first, and
//! each filter targets the one after it, the last one the source.
//!
//! - [`compatible`] is `filter_compatible()`: a filter's audio and async
//!   video bits must be a subset of the source's, without the async bit for
//!   a filter that has audio and no video.
//! - `obs_source_filter_add()` asks [`add_allowed`] after the list check,
//!   [`checks_compatible`] before [`compatible`], [`ref_step`] after
//!   referencing the filter, and [`add_target`] for the new first filter's
//!   target (the source, or the old first filter).
//! - `obs_source_filter_remove_refless()` asks [`remove_retarget`] which
//!   filter takes the removed one's target (the one before it).
//! - `move_filter_dir()` asks [`move_route`] for the movement. Up and down
//!   walk to the nearest filter of the same kind (async or not), one
//!   [`walk_step`] and one [`same_kind`] per filter; top and bottom move to
//!   an end whatever the kind ([`edge_move`]). Every filter is then
//!   retargeted ([`target_of`]).
//! - `obs_source_restore_filters()` keeps, for each backed-up filter, the
//!   first current filter whose name [`restore_matches`] its own, and asks
//!   [`restore_step`] how to get one that has none.
//!
//! `obs-source.c.h` keeps every `filter_mutex` lock, every list edit, every
//! `filter_parent` and `filter_target` write, every reference and release,
//! the loads, every `obs_data` call, the `filter_add` and `filter_remove`
//! callbacks, every signal and every log, each where it was, and reads each
//! value where it did. Nothing here keeps state, takes a lock, calls libobs
//! or follows a pointer.
//!
//! Four intentional differences from the C, each a fix its caller follows:
//! - [`add_allowed`] refuses a filter that has a parent: the C took a
//!   filter that is on another source, which then sat in both chains;
//! - [`ref_step`] unlocks before returning when the filter can't be
//!   referenced: the C returned with `filter_mutex` still locked, and every
//!   later filter call on the source blocked;
//! - [`move_route`] gives [`Route::Stay`] for a movement outside
//!   `enum obs_order_movement`: the C moved nothing but retargeted every
//!   filter and reported a move, so `reorder_filters` was signalled;
//! - [`restore_step`] loads a backed-up filter that has no current one
//!   private: the C loaded it public, so a filter whose plugin is missing
//!   came back as a public source.

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

/// `OBS_SOURCE_VIDEO` (`info.output_flags`).
pub const VIDEO: u32 = 1 << 0;
/// `OBS_SOURCE_AUDIO`.
pub const AUDIO: u32 = 1 << 1;
/// `OBS_SOURCE_ASYNC`.
pub const ASYNC: u32 = 1 << 2;

/// `OBS_ORDER_MOVE_UP` (`enum obs_order_movement`).
pub const MOVE_UP: i32 = 0;
/// `OBS_ORDER_MOVE_DOWN`.
pub const MOVE_DOWN: i32 = 1;
/// `OBS_ORDER_MOVE_TOP`.
pub const MOVE_TOP: i32 = 2;
/// `OBS_ORDER_MOVE_BOTTOM`.
pub const MOVE_BOTTOM: i32 = 3;

/// `enum obs_source_filter_chain_ref`: what `obs_source_filter_add()` does
/// after referencing the filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefStep {
    /// Set the parent and target and insert the filter.
    Add,
    /// Return with `filter_mutex` still locked (the C's answer; never given
    /// here).
    Return,
    /// Unlock `filter_mutex`, then return.
    UnlockReturn,
}

/// `enum obs_source_filter_chain_target`: what a filter targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    /// The source the filter is on.
    Source,
    /// The filter after it (for a new filter, the old first one).
    Filter,
}

/// `enum obs_source_filter_chain_route`: what `move_filter_dir()` does for
/// a movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Route {
    /// Walk up to the nearest filter of the same kind.
    Up,
    /// Walk down to the nearest filter of the same kind.
    Down,
    /// Move to the top (the end of the list).
    Top,
    /// Move to the bottom (the start of the list).
    Bottom,
    /// Move nothing, but retarget every filter and report a move (the C's
    /// answer; never given here).
    Retarget,
    /// Move nothing and report no move.
    Stay,
}

/// `enum obs_source_filter_chain_restore`: how
/// `obs_source_restore_filters()` gets a backed-up filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Restore {
    /// Keep the current filter it found.
    Reuse,
    /// Load it public (the C's answer; never given here).
    Load,
    /// Load it private.
    LoadPrivate,
}

/// `obs_source_filter_chain_compatible()`: whether a filter with
/// `filter_flags` fits a source with `source_flags`.
pub const fn compatible(source_flags: u32, filter_flags: u32) -> bool {
    let av = VIDEO | AUDIO | ASYNC;
    let s_caps = source_flags & av;
    let mut f_caps = filter_flags & av;

    if f_caps & AUDIO != 0 && f_caps & VIDEO == 0 {
        f_caps &= !ASYNC;
    }

    s_caps & f_caps == f_caps
}

/// `obs_source_filter_chain_add_allowed()`: a filter goes on only when it
/// is not `listed` on the source and has no parent (the C ignored
/// `has_parent`).
pub const fn add_allowed(listed: bool, has_parent: bool) -> bool {
    !listed && !has_parent
}

/// `obs_source_filter_chain_checks_compatible()`: the compatibility check
/// is skipped for a source that owns its info id.
pub const fn checks_compatible(owns_info_id: bool) -> bool {
    !owns_info_id
}

/// `obs_source_filter_chain_ref_step()`: [`RefStep::Add`] when the filter
/// was referenced, else [`RefStep::UnlockReturn`].
pub const fn ref_step(referenced: bool) -> RefStep {
    if referenced {
        RefStep::Add
    } else {
        RefStep::UnlockReturn
    }
}

/// `obs_source_filter_chain_add_target()`: a new filter targets the source
/// on an empty list (`count` 0), else the old first filter.
pub const fn add_target(count: usize) -> Target {
    if count == 0 {
        Target::Source
    } else {
        Target::Filter
    }
}

/// `obs_source_filter_chain_remove_retarget()`: the index of the filter
/// that takes the removed filter's target (the one before `idx`), if any.
pub const fn remove_retarget(idx: usize) -> Option<usize> {
    idx.checked_sub(1)
}

/// `obs_source_filter_chain_move_route()`: the route of a movement;
/// [`Route::Stay`] for one outside the enum.
pub const fn move_route(movement: i32) -> Route {
    match movement {
        MOVE_UP => Route::Up,
        MOVE_DOWN => Route::Down,
        MOVE_TOP => Route::Top,
        MOVE_BOTTOM => Route::Bottom,
        _ => Route::Stay,
    }
}

/// `obs_source_filter_chain_walk_step()`: from index `cur` of `count`
/// filters, the next index of the walk (`cur + 1` going `up`, else
/// `cur - 1`), or `None` at the end of the list. Like the C's `size_t`, the
/// last index of an empty list is `usize::MAX`.
pub const fn walk_step(up: bool, cur: usize, count: usize) -> Option<usize> {
    if up {
        if cur == count.wrapping_sub(1) {
            None
        } else {
            Some(cur.wrapping_add(1))
        }
    } else {
        cur.checked_sub(1)
    }
}

/// `obs_source_filter_chain_same_kind()`: whether both flag words have
/// [`ASYNC`] or neither has.
pub const fn same_kind(filter_flags: u32, other_flags: u32) -> bool {
    (filter_flags & ASYNC != 0) == (other_flags & ASYNC != 0)
}

/// `obs_source_filter_chain_edge_move()`: where the filter at `idx` of
/// `count` goes for the top (`top`) or the bottom, or `None` when it is
/// already there.
pub const fn edge_move(top: bool, idx: usize, count: usize) -> Option<usize> {
    if top {
        let last = count.wrapping_sub(1);
        if idx == last { None } else { Some(last) }
    } else if idx == 0 {
        None
    } else {
        Some(0)
    }
}

/// `obs_source_filter_chain_target_of()`: the last of `count` filters
/// targets the source, any other the next filter.
pub const fn target_of(i: usize, count: usize) -> Target {
    if i == count.wrapping_sub(1) {
        Target::Source
    } else {
        Target::Filter
    }
}

/// `obs_source_filter_chain_restore_matches()`: whether a current filter
/// named `cur_name` (`None` for no name) is the backed-up filter `name`.
pub fn restore_matches(cur_name: Option<&[u8]>, name: &[u8]) -> bool {
    cur_name == Some(name)
}

/// `obs_source_filter_chain_restore_step()`: [`Restore::Reuse`] when a
/// current filter was found, else [`Restore::LoadPrivate`].
pub const fn restore_step(found: bool) -> Restore {
    if found {
        Restore::Reuse
    } else {
        Restore::LoadPrivate
    }
}
