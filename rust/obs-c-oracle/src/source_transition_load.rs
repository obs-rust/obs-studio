//! `oracle_*` build of `libobs/obs-source-transition-load.c.h`
//! (oracle/source_transition_load.c): the load, scale type and swap checks
//! of the transitions, renamed. The `bool` results are declared as `u8`, the
//! byte the C returns, so a parity test compares it without assuming it is 0
//! or 1; the source outcome as `u32` (the C's enum, an `int` or `unsigned
//! int`; every value fits both), the source type and the scale type result
//! as [`c_int`], the loaded scale type as `i64`.

use std::os::raw::c_int;

/// Index of each entry in `oracle_source_transition_load_value`, in the
/// C's order.
pub const VALUE_NAMES: [&str; 10] = [
    "OBS_SOURCE_TRANSITION_LOAD_TYPE_TRANSITION",
    "OBS_SOURCE_TRANSITION_LOAD_SCALE_MAX_ONLY",
    "OBS_SOURCE_TRANSITION_LOAD_SCALE_ASPECT",
    "OBS_SOURCE_TRANSITION_LOAD_SCALE_STRETCH",
    "OBS_SOURCE_TRANSITION_LOAD_KEEP",
    "OBS_SOURCE_TRANSITION_LOAD_NOT_FOUND",
    "OBS_SOURCE_TRANSITION_LOAD_REFUSE",
    "sizeof(enum obs_source_transition_load_outcome)",
    "sizeof(bool)",
    "sizeof(int)",
];

unsafe extern "C" {
    /// `bool obs_source_transition_load_is_transition(int type)`.
    pub fn oracle_obs_source_transition_load_is_transition(source_type: c_int) -> u8;
    /// `int obs_source_transition_load_scale_type(int64_t type)`.
    pub fn oracle_obs_source_transition_load_scale_type(value: i64) -> c_int;
    /// `bool obs_source_transition_load_looks_up(bool has_name)`.
    pub fn oracle_obs_source_transition_load_looks_up(has_name: bool) -> u8;
    /// `enum obs_source_transition_load_outcome
    /// obs_source_transition_load_source(bool found, bool add_ok)`.
    pub fn oracle_obs_source_transition_load_source(found: bool, add_ok: bool) -> u32;
    /// `bool obs_source_transition_load_swaps(bool same)`.
    pub fn oracle_obs_source_transition_load_swaps(same: bool) -> u8;
    /// `bool obs_source_transition_load_copy_stores(bool adds, bool
    /// add_ok)`.
    pub fn oracle_obs_source_transition_load_copy_stores(adds: bool, add_ok: bool) -> u8;
    /// The header's values and sizes, by index into [`VALUE_NAMES`];
    /// `INT64_MIN` past the end.
    pub fn oracle_source_transition_load_value(which: usize) -> i64;
}
