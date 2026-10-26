//! `oracle_*` build of `libobs/obs-output-stop.c.h` (oracle/output_stop.c):
//! the output stop and end-capture decisions, renamed.
//!
//! Every function takes plain values. Nothing here divides.

use std::ffi::{c_int, c_long};

/// Index of each entry in `oracle_output_stop_constant`, in the C's order.
pub const CONSTANT_NAMES: [&str; 6] = [
    "OBS_OUTPUT_STOP_VIDEO",
    "OBS_OUTPUT_STOP_AUDIO",
    "OBS_OUTPUT_STOP_ENCODED",
    "OBS_OUTPUT_STOP_SERVICE",
    "sizeof(long)",
    "sizeof(int)",
];

unsafe extern "C" {
    pub fn oracle_obs_output_stop_encoded(flags: u32) -> bool;

    pub fn oracle_obs_output_stop_delay_on(active_delay_ns: u64) -> bool;

    pub fn oracle_obs_output_stop_has_video(flags: u32) -> bool;

    pub fn oracle_obs_output_stop_has_audio(flags: u32) -> bool;

    pub fn oracle_obs_output_stop_service(flags: u32) -> bool;

    pub fn oracle_obs_output_stop_end_delay(delay_restart_refs: c_long) -> bool;

    pub fn oracle_obs_output_stop_thread_failed(ret: c_int) -> bool;

    /// The constant as the C sees it, by index into [`CONSTANT_NAMES`]; -1
    /// past the end.
    pub fn oracle_output_stop_constant(which: usize) -> i64;
}
