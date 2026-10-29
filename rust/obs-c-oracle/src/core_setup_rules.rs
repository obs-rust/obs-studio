//! `oracle_*` build of `libobs/obs-core-setup-rules.c.h`
//! (oracle/core_setup_rules.c).
//!
//! The C's enums are 32-bit integers, taken and returned as `u32` as the
//! shims do. The constants and enum values of `obs-core-setup-rules.h`, and
//! the `obs-defs.h`, `obs-source.h` and `audio-io.h` values it copies, come
//! from the same C file.

use core::ffi::c_int;

/// Index of each entry in `oracle_core_setup_rules_constant`, in the C's
/// order.
pub const CONSTANT_NAMES: [&str; 31] = [
    "OBS_CORE_SETUP_RULES_VIDEO_GO",
    "OBS_CORE_SETUP_RULES_VIDEO_FAIL",
    "OBS_CORE_SETUP_RULES_VIDEO_INVALID_PARAM",
    "OBS_CORE_SETUP_RULES_VIDEO_CURRENTLY_ACTIVE",
    "OBS_VIDEO_SUCCESS",
    "OBS_VIDEO_FAIL",
    "OBS_VIDEO_INVALID_PARAM",
    "OBS_VIDEO_CURRENTLY_ACTIVE",
    "OBS_CORE_SETUP_RULES_DO_NOT_SELF_MONITOR",
    "OBS_SOURCE_DO_NOT_SELF_MONITOR",
    "OBS_CORE_SETUP_RULES_AUDIO_OUTPUT_FRAMES",
    "AUDIO_OUTPUT_FRAMES",
    "OBS_CORE_SETUP_RULES_DEFAULT_TICKS",
    "OBS_CORE_SETUP_RULES_LATEST_NONE",
    "OBS_CORE_SETUP_RULES_VIDEO_CORE",
    "OBS_CORE_SETUP_RULES_VIDEO_ACTIVE",
    "OBS_CORE_SETUP_RULES_VIDEO_STARTING",
    "OBS_CORE_SETUP_RULES_VIDEO_OUTPUT_SIZE",
    "OBS_CORE_SETUP_RULES_VIDEO_BASE_SIZE",
    "OBS_CORE_SETUP_RULES_AUDIO_CORE",
    "OBS_CORE_SETUP_RULES_AUDIO_ACTIVE",
    "OBS_CORE_SETUP_RULES_MIX_MAIN",
    "OBS_CORE_SETUP_RULES_MIX_ENCODER_ONLY",
    "OBS_CORE_SETUP_RULES_MIX_VIEW",
    "OBS_CORE_SETUP_RULES_MIX_VIDEO",
    "OBS_CORE_SETUP_RULES_MIX_COUNT",
    "OBS_CORE_SETUP_RULES_MIX_SKIP",
    "sizeof(enum obs_core_setup_rules_video_step)",
    "sizeof(enum obs_core_setup_rules_audio_step)",
    "sizeof(enum obs_core_setup_rules_mix)",
    "sizeof(void *)",
];

unsafe extern "C" {
    pub fn oracle_obs_core_setup_rules_video_gate(step: u32, value: bool) -> c_int;
    pub fn oracle_obs_core_setup_rules_audio_refused(step: u32, value: bool) -> bool;
    pub fn oracle_obs_core_setup_rules_buffer_set(max_buffering_ms: u32) -> bool;
    pub fn oracle_obs_core_setup_rules_buffer_ticks(
        max_buffering_ms: u32,
        samples_per_sec: u32,
    ) -> u32;
    pub fn oracle_obs_core_setup_rules_buffer_default_ticks() -> u32;
    pub fn oracle_obs_core_setup_rules_cmdline_set(stored_argc: c_int) -> bool;
    pub fn oracle_obs_core_setup_rules_cmdline_len(len: usize, arg_len: usize) -> usize;
    pub fn oracle_obs_core_setup_rules_cmdline_table(argc: c_int, ptr_size: usize) -> usize;
    pub fn oracle_obs_core_setup_rules_data_file_found(found: bool) -> bool;
    pub fn oracle_obs_core_setup_rules_data_path_match(cmp: c_int) -> bool;
    pub fn oracle_obs_core_setup_rules_latest_newer(version: u32, best: c_int) -> bool;
    pub fn oracle_obs_core_setup_rules_aoc_check(output_flags: u32) -> bool;
    pub fn oracle_obs_core_setup_rules_monitor_missing(present: bool) -> bool;
    pub fn oracle_obs_core_setup_rules_monitor_same(cmp: c_int) -> bool;
    pub fn oracle_obs_core_setup_rules_mix_step(step: u32, value: bool) -> u32;
    pub fn oracle_core_setup_rules_constant(i: usize) -> i64;
}

/// The C's value of `CONSTANT_NAMES[i]`.
pub fn constant(i: usize) -> i64 {
    // SAFETY: the oracle answers any index.
    unsafe { oracle_core_setup_rules_constant(i) }
}
