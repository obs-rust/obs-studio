//! `oracle_*` build of `libobs/obs-output-start.c.h`
//! (oracle/output_start.c): the output start and begin-capture decisions,
//! renamed.
//!
//! Every function takes plain values. Nothing here divides.

use std::ffi::c_long;

/// Index of each entry in `oracle_output_start_constant`, in the C's order.
pub const CONSTANT_NAMES: [&str; 5] = [
    "OBS_OUTPUT_START_VIDEO",
    "OBS_OUTPUT_START_AUDIO",
    "OBS_OUTPUT_START_ENCODED",
    "OBS_OUTPUT_START_SERVICE",
    "sizeof(long)",
];

unsafe extern "C" {
    pub fn oracle_obs_output_start_service(flags: u32) -> bool;

    pub fn oracle_obs_output_start_drop_restart_ref(delay_restart_refs: c_long) -> bool;

    pub fn oracle_obs_output_start_init_video(flags: u32) -> bool;

    pub fn oracle_obs_output_start_init_audio(flags: u32) -> bool;

    pub fn oracle_obs_output_start_reset_raw(flags: u32) -> bool;

    pub fn oracle_obs_output_start_pair(flags: u32) -> bool;

    /// The constant as the C sees it, by index into [`CONSTANT_NAMES`]; -1
    /// past the end.
    pub fn oracle_output_start_constant(which: usize) -> i64;
}
