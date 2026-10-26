//! `oracle_*` build of `libobs/obs-source-transition-start.c.h`
//! (oracle/source_transition_start.c): the start checks and child handover
//! of the transitions, renamed. The `bool` results are declared as `u8`,
//! the byte the C returns, so a parity test compares it without assuming
//! it is 0 or 1; the admission as `u32` (the C's enum, an `int` or
//! `unsigned int`; every value fits both), `size_t` as `usize`, the mode as
//! [`c_int`].

use std::os::raw::c_int;

/// Index of each entry in `oracle_source_transition_start_value`, in the
/// C's order.
pub const VALUE_NAMES: [&str; 7] = [
    "OBS_SOURCE_TRANSITION_START_MODE_MANUAL",
    "OBS_SOURCE_TRANSITION_START_GO",
    "OBS_SOURCE_TRANSITION_START_REFUSE",
    "OBS_SOURCE_TRANSITION_START_DONE",
    "sizeof(enum obs_source_transition_start_admission)",
    "sizeof(bool)",
    "sizeof(size_t)",
];

unsafe extern "C" {
    /// `bool obs_source_transition_start_refused(bool same_as_source, bool
    /// active)`.
    pub fn oracle_obs_source_transition_start_refused(same_as_source: bool, active: bool) -> u8;
    /// `bool obs_source_transition_start_walks(bool has_dest, bool
    /// recursive)`.
    pub fn oracle_obs_source_transition_start_walks(has_dest: bool, recursive: bool) -> u8;
    /// `enum obs_source_transition_start_admission
    /// obs_source_transition_start_admit(bool same_as_source, bool
    /// same_as_dest, bool same_mode, int mode, bool active)`.
    pub fn oracle_obs_source_transition_start_admit(
        same_as_source: bool,
        same_as_dest: bool,
        same_mode: bool,
        mode: c_int,
        active: bool,
    ) -> u32;
    /// `uint64_t obs_source_transition_start_duration_ns(uint32_t ms)`.
    pub fn oracle_obs_source_transition_start_duration_ns(ms: u32) -> u64;
    /// `bool obs_source_transition_start_restart(bool active, bool
    /// same_as_dest, bool same_as_source)`.
    pub fn oracle_obs_source_transition_start_restart(
        active: bool,
        same_as_dest: bool,
        same_as_source: bool,
    ) -> u8;
    /// `bool obs_source_transition_start_adds(bool already_active, bool
    /// has_new)`.
    pub fn oracle_obs_source_transition_start_adds(already_active: bool, has_new: bool) -> u8;
    /// `bool obs_source_transition_start_removes(bool already_active, bool
    /// has_old, bool add_ok)`.
    pub fn oracle_obs_source_transition_start_removes(
        already_active: bool,
        has_old: bool,
        add_ok: bool,
    ) -> u8;
    /// `bool obs_source_transition_start_calls_back(bool has_callback, bool
    /// add_ok)`.
    pub fn oracle_obs_source_transition_start_calls_back(has_callback: bool, add_ok: bool) -> u8;
    /// `bool obs_source_transition_start_unsized(uint32_t size)`.
    pub fn oracle_obs_source_transition_start_unsized(size: u32) -> u8;
    /// `bool obs_source_transition_start_null_dest(bool has_dest, bool
    /// same_as_dest, bool same_as_source)`.
    pub fn oracle_obs_source_transition_start_null_dest(
        has_dest: bool,
        same_as_dest: bool,
        same_as_source: bool,
    ) -> u8;
    /// `bool obs_source_transition_start_activates(bool slot_active)`.
    pub fn oracle_obs_source_transition_start_activates(slot_active: bool) -> u8;
    /// `size_t obs_source_transition_start_active_index(bool active)`.
    pub fn oracle_obs_source_transition_start_active_index(active: bool) -> usize;
    /// `bool obs_source_transition_start_manual_reset(int mode, float t)`.
    pub fn oracle_obs_source_transition_start_manual_reset(mode: c_int, t: f32) -> u8;
    /// `bool obs_source_transition_start_child_active(bool has_child, bool
    /// active)`.
    pub fn oracle_obs_source_transition_start_child_active(has_child: bool, active: bool) -> u8;
    /// The header's values and sizes, by index into [`VALUE_NAMES`];
    /// `INT64_MIN` past the end.
    pub fn oracle_source_transition_start_value(which: usize) -> i64;
}
