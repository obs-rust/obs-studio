//! `oracle_*` build of `libobs/obs-source-filter-chain.c.h`
//! (oracle/source_filter_chain.c): the filter chain decisions of
//! `filter_compatible()`, `obs_source_filter_add()`,
//! `obs_source_filter_remove_refless()`, `move_filter_dir()` (with its walks
//! and `reorder_filter_targets()`) and `obs_source_restore_filters()`,
//! renamed.

use std::ffi::{c_char, c_int};

unsafe extern "C" {
    pub fn oracle_obs_source_filter_chain_compatible(source_flags: u32, filter_flags: u32) -> bool;

    pub fn oracle_obs_source_filter_chain_add_allowed(listed: bool, has_parent: bool) -> bool;

    pub fn oracle_obs_source_filter_chain_checks_compatible(owns_info_id: bool) -> bool;

    pub fn oracle_obs_source_filter_chain_ref_step(referenced: bool) -> c_int;

    pub fn oracle_obs_source_filter_chain_add_target(count: usize) -> c_int;

    pub fn oracle_obs_source_filter_chain_remove_retarget(idx: usize, prev: *mut usize) -> bool;

    pub fn oracle_obs_source_filter_chain_move_route(movement: c_int) -> c_int;

    pub fn oracle_obs_source_filter_chain_walk_step(
        route: c_int,
        cur: usize,
        count: usize,
        next: *mut usize,
    ) -> bool;

    pub fn oracle_obs_source_filter_chain_same_kind(filter_flags: u32, other_flags: u32) -> bool;

    pub fn oracle_obs_source_filter_chain_edge_move(
        route: c_int,
        idx: usize,
        count: usize,
        to: *mut usize,
    ) -> bool;

    pub fn oracle_obs_source_filter_chain_target_of(i: usize, count: usize) -> c_int;

    pub fn oracle_obs_source_filter_chain_restore_matches(
        cur_name: *const c_char,
        name: *const c_char,
    ) -> bool;

    pub fn oracle_obs_source_filter_chain_restore_step(found: bool) -> c_int;

    /// The values of `oracle/source_filter_chain.c`, by index: the video,
    /// audio and async flags, the four movements; the three ref steps, the
    /// two targets, the six routes, the three restore steps; the sizes of
    /// the four enums, of `bool` and of `size_t`.
    pub fn oracle_source_filter_chain_layout(which: usize) -> usize;
}
