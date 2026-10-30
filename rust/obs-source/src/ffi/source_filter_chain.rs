//! C ABI for `libobs/obs-source-filter-chain.c.h`, the filter chain
//! decisions of `filter_compatible()`, `obs_source_filter_add()`,
//! `obs_source_filter_remove_refless()`, `move_filter_dir()` (with
//! `find_next_filter()`, `find_prev_filter()` and
//! `reorder_filter_targets()`) and `obs_source_restore_filters()`, declared
//! in `libobs/obs-source-filter-chain.h` without `EXPORT`.
//!
//! `obs-source.c.h` (still C) makes every lock, list edit, read, reference
//! and load itself and passes what it found as flags, indices, counts and,
//! for [`obs_source_filter_chain_restore_matches`], two C strings that are
//! only read here. So no libobs structure is mirrored, and these shims take
//! no lock and call nothing in libobs. All thirteen stay hidden
//! (`rust-hidden.txt`). The enum results are returned as `int`, as the
//! header declares them.
//!
//! Four intentional differences from C, followed by the callers:
//! - [`obs_source_filter_chain_add_allowed`] refuses a filter that has a
//!   parent (the C took a filter that is on another source);
//! - [`obs_source_filter_chain_ref_step`] gives
//!   `OBS_SOURCE_FILTER_CHAIN_REF_UNLOCK_RETURN` for a filter that can't be
//!   referenced, and `obs_source_filter_add()` unlocks `filter_mutex`
//!   before it returns (the C gave `OBS_SOURCE_FILTER_CHAIN_REF_RETURN` and
//!   returned with it locked);
//! - [`obs_source_filter_chain_move_route`] gives
//!   `OBS_SOURCE_FILTER_CHAIN_ROUTE_STAY` for a movement outside
//!   `enum obs_order_movement`, and `move_filter_dir()` reports no move
//!   (the C gave `OBS_SOURCE_FILTER_CHAIN_ROUTE_RETARGET`: every filter
//!   retargeted and `reorder_filters` signalled);
//! - [`obs_source_filter_chain_restore_step`] gives
//!   `OBS_SOURCE_FILTER_CHAIN_RESTORE_LOAD_PRIVATE`, and
//!   `obs_source_restore_filters()` loads with `obs_load_private_source()`
//!   (the C gave `OBS_SOURCE_FILTER_CHAIN_RESTORE_LOAD`, a public load).

use std::ffi::{CStr, c_char};
use std::os::raw::c_int;

use crate::source_filter_chain::{self as core, RefStep, Restore, Route, Target};

/// `OBS_SOURCE_FILTER_CHAIN_REF_ADD`.
pub const OBS_SOURCE_FILTER_CHAIN_REF_ADD: c_int = 0;
/// `OBS_SOURCE_FILTER_CHAIN_REF_RETURN`.
pub const OBS_SOURCE_FILTER_CHAIN_REF_RETURN: c_int = 1;
/// `OBS_SOURCE_FILTER_CHAIN_REF_UNLOCK_RETURN`.
pub const OBS_SOURCE_FILTER_CHAIN_REF_UNLOCK_RETURN: c_int = 2;

/// `OBS_SOURCE_FILTER_CHAIN_TARGET_SOURCE`.
pub const OBS_SOURCE_FILTER_CHAIN_TARGET_SOURCE: c_int = 0;
/// `OBS_SOURCE_FILTER_CHAIN_TARGET_FILTER`.
pub const OBS_SOURCE_FILTER_CHAIN_TARGET_FILTER: c_int = 1;

/// `OBS_SOURCE_FILTER_CHAIN_ROUTE_UP`.
pub const OBS_SOURCE_FILTER_CHAIN_ROUTE_UP: c_int = 0;
/// `OBS_SOURCE_FILTER_CHAIN_ROUTE_DOWN`.
pub const OBS_SOURCE_FILTER_CHAIN_ROUTE_DOWN: c_int = 1;
/// `OBS_SOURCE_FILTER_CHAIN_ROUTE_TOP`.
pub const OBS_SOURCE_FILTER_CHAIN_ROUTE_TOP: c_int = 2;
/// `OBS_SOURCE_FILTER_CHAIN_ROUTE_BOTTOM`.
pub const OBS_SOURCE_FILTER_CHAIN_ROUTE_BOTTOM: c_int = 3;
/// `OBS_SOURCE_FILTER_CHAIN_ROUTE_RETARGET`.
pub const OBS_SOURCE_FILTER_CHAIN_ROUTE_RETARGET: c_int = 4;
/// `OBS_SOURCE_FILTER_CHAIN_ROUTE_STAY`.
pub const OBS_SOURCE_FILTER_CHAIN_ROUTE_STAY: c_int = 5;

/// `OBS_SOURCE_FILTER_CHAIN_RESTORE_REUSE`.
pub const OBS_SOURCE_FILTER_CHAIN_RESTORE_REUSE: c_int = 0;
/// `OBS_SOURCE_FILTER_CHAIN_RESTORE_LOAD`.
pub const OBS_SOURCE_FILTER_CHAIN_RESTORE_LOAD: c_int = 1;
/// `OBS_SOURCE_FILTER_CHAIN_RESTORE_LOAD_PRIVATE`.
pub const OBS_SOURCE_FILTER_CHAIN_RESTORE_LOAD_PRIVATE: c_int = 2;

const fn ref_value(step: RefStep) -> c_int {
    match step {
        RefStep::Add => OBS_SOURCE_FILTER_CHAIN_REF_ADD,
        RefStep::Return => OBS_SOURCE_FILTER_CHAIN_REF_RETURN,
        RefStep::UnlockReturn => OBS_SOURCE_FILTER_CHAIN_REF_UNLOCK_RETURN,
    }
}

const fn target_value(target: Target) -> c_int {
    match target {
        Target::Source => OBS_SOURCE_FILTER_CHAIN_TARGET_SOURCE,
        Target::Filter => OBS_SOURCE_FILTER_CHAIN_TARGET_FILTER,
    }
}

const fn route_value(route: Route) -> c_int {
    match route {
        Route::Up => OBS_SOURCE_FILTER_CHAIN_ROUTE_UP,
        Route::Down => OBS_SOURCE_FILTER_CHAIN_ROUTE_DOWN,
        Route::Top => OBS_SOURCE_FILTER_CHAIN_ROUTE_TOP,
        Route::Bottom => OBS_SOURCE_FILTER_CHAIN_ROUTE_BOTTOM,
        Route::Retarget => OBS_SOURCE_FILTER_CHAIN_ROUTE_RETARGET,
        Route::Stay => OBS_SOURCE_FILTER_CHAIN_ROUTE_STAY,
    }
}

const fn restore_value(restore: Restore) -> c_int {
    match restore {
        Restore::Reuse => OBS_SOURCE_FILTER_CHAIN_RESTORE_REUSE,
        Restore::Load => OBS_SOURCE_FILTER_CHAIN_RESTORE_LOAD,
        Restore::LoadPrivate => OBS_SOURCE_FILTER_CHAIN_RESTORE_LOAD_PRIVATE,
    }
}

/// Stores `value` in `*out` when it is `Some`, and says whether it was.
///
/// # Safety
///
/// `out` must be valid for a write of one `usize` when `value` is `Some`.
unsafe fn store(value: Option<usize>, out: *mut usize) -> bool {
    let Some(value) = value else {
        return false;
    };
    // SAFETY: the C caller passes the address of a local `size_t`, valid
    // for writes; `size_t` is `usize` on every target libobs builds for.
    unsafe { out.write(value) };
    true
}

/// `bool obs_source_filter_chain_compatible(uint32_t source_flags, uint32_t
/// filter_flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_filter_chain_compatible(source_flags: u32, filter_flags: u32) -> bool {
    core::compatible(source_flags, filter_flags)
}

/// `bool obs_source_filter_chain_add_allowed(bool listed, bool
/// has_parent)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_filter_chain_add_allowed(listed: bool, has_parent: bool) -> bool {
    core::add_allowed(listed, has_parent)
}

/// `bool obs_source_filter_chain_checks_compatible(bool owns_info_id)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_filter_chain_checks_compatible(owns_info_id: bool) -> bool {
    core::checks_compatible(owns_info_id)
}

/// `int obs_source_filter_chain_ref_step(bool referenced)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_filter_chain_ref_step(referenced: bool) -> c_int {
    ref_value(core::ref_step(referenced))
}

/// `int obs_source_filter_chain_add_target(size_t count)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_filter_chain_add_target(count: usize) -> c_int {
    target_value(core::add_target(count))
}

/// `bool obs_source_filter_chain_remove_retarget(size_t idx, size_t
/// *prev)`. `*prev` is written only when it returns true.
///
/// # Safety
///
/// `prev` must be valid for a write of one `size_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_filter_chain_remove_retarget(
    idx: usize,
    prev: *mut usize,
) -> bool {
    // SAFETY: the caller's contract.
    unsafe { store(core::remove_retarget(idx), prev) }
}

/// `int obs_source_filter_chain_move_route(int movement)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_filter_chain_move_route(movement: c_int) -> c_int {
    route_value(core::move_route(movement))
}

/// `bool obs_source_filter_chain_walk_step(int route, size_t cur, size_t
/// count, size_t *next)`. As in the C, any route but
/// `OBS_SOURCE_FILTER_CHAIN_ROUTE_UP` walks down. `*next` is written only
/// when it returns true.
///
/// # Safety
///
/// `next` must be valid for a write of one `size_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_filter_chain_walk_step(
    route: c_int,
    cur: usize,
    count: usize,
    next: *mut usize,
) -> bool {
    let up = route == OBS_SOURCE_FILTER_CHAIN_ROUTE_UP;
    // SAFETY: the caller's contract.
    unsafe { store(core::walk_step(up, cur, count), next) }
}

/// `bool obs_source_filter_chain_same_kind(uint32_t filter_flags, uint32_t
/// other_flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_filter_chain_same_kind(filter_flags: u32, other_flags: u32) -> bool {
    core::same_kind(filter_flags, other_flags)
}

/// `bool obs_source_filter_chain_edge_move(int route, size_t idx, size_t
/// count, size_t *to)`. As in the C, any route but
/// `OBS_SOURCE_FILTER_CHAIN_ROUTE_TOP` goes to the bottom. `*to` is written
/// only when it returns true.
///
/// # Safety
///
/// `to` must be valid for a write of one `size_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_filter_chain_edge_move(
    route: c_int,
    idx: usize,
    count: usize,
    to: *mut usize,
) -> bool {
    let top = route == OBS_SOURCE_FILTER_CHAIN_ROUTE_TOP;
    // SAFETY: the caller's contract.
    unsafe { store(core::edge_move(top, idx, count), to) }
}

/// `int obs_source_filter_chain_target_of(size_t i, size_t count)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_filter_chain_target_of(i: usize, count: usize) -> c_int {
    target_value(core::target_of(i, count))
}

/// `bool obs_source_filter_chain_restore_matches(const char *cur_name,
/// const char *name)`. A NULL `cur_name` never matches; nor does a NULL
/// `name`, which the C never passes (`strcmp()` on it is undefined).
///
/// # Safety
///
/// Each pointer must be NULL or point to a NUL-terminated string that
/// stays valid for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_filter_chain_restore_matches(
    cur_name: *const c_char,
    name: *const c_char,
) -> bool {
    if name.is_null() {
        return false;
    }
    // SAFETY: non-NULL, and NUL-terminated by the caller's contract.
    let name = unsafe { CStr::from_ptr(name) };
    let cur_name = if cur_name.is_null() {
        None
    } else {
        // SAFETY: non-NULL, and NUL-terminated by the caller's contract.
        Some(unsafe { CStr::from_ptr(cur_name) }.to_bytes())
    };
    core::restore_matches(cur_name, name.to_bytes())
}

/// `int obs_source_filter_chain_restore_step(bool found)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_filter_chain_restore_step(found: bool) -> c_int {
    restore_value(core::restore_step(found))
}
