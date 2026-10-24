//! `oracle_*` build of `libobs/obs-source-transition-clock.c.h`
//! (oracle/source_transition_clock.c): the clock and fit decisions of the
//! transitions, renamed. Everything crosses as plain values, and the fit's
//! scale through two `float *`.

use core::ffi::c_int;

/// Index of each entry in `oracle_source_transition_clock_layout`, in the
/// C's order.
pub const LAYOUT_NAMES: [&str; 17] = [
    "OBS_SOURCE_TRANSITION_CLOCK_MODE_MANUAL",
    "OBS_SOURCE_TRANSITION_CLOCK_SCALE_MAX_ONLY",
    "OBS_SOURCE_TRANSITION_CLOCK_SCALE_ASPECT",
    "OBS_SOURCE_TRANSITION_CLOCK_SCALE_STRETCH",
    "OBS_SOURCE_TRANSITION_CLOCK_CS_SRGB",
    "OBS_SOURCE_TRANSITION_CLOCK_CS_SRGB_16F",
    "OBS_SOURCE_TRANSITION_CLOCK_CS_709_EXTENDED",
    "OBS_SOURCE_TRANSITION_CLOCK_CS_709_SCRGB",
    "sizeof(enum obs_source_transition_clock_step)",
    "OBS_SOURCE_TRANSITION_CLOCK_HOLD",
    "OBS_SOURCE_TRANSITION_CLOCK_SNAP",
    "OBS_SOURCE_TRANSITION_CLOCK_STEP",
    "sizeof(int)",
    "sizeof(bool)",
    "sizeof(float)",
    "LDBL_MANT_DIG",
    "FLT_EVAL_METHOD",
];

/// The index of `LDBL_MANT_DIG` in [`LAYOUT_NAMES`].
pub const LDBL_MANT_DIG_INDEX: usize = 15;

unsafe extern "C" {
    /// `obs_source_transition_clock_time`.
    pub fn oracle_obs_source_transition_clock_time(
        mode: c_int,
        manual_val: f32,
        start_time: u64,
        duration: u64,
        ts: u64,
    ) -> f32;
    /// `obs_source_transition_clock_manual`: the step as the C's enum, an
    /// `int` or `unsigned int` (every value fits both).
    pub fn oracle_obs_source_transition_clock_manual(mode: c_int, torque: f32) -> u32;
    /// `obs_source_transition_clock_size`.
    pub fn oracle_obs_source_transition_clock_size(set: u32, actual: u32) -> u32;
    /// `obs_source_transition_clock_fold`.
    pub fn oracle_obs_source_transition_clock_fold(size: u32, child: u32) -> u32;
    /// `obs_source_transition_clock_fit`: writes through `scale_x` and
    /// `scale_y` (both valid) only where it gives a scale.
    pub fn oracle_obs_source_transition_clock_fit(
        tr_cx: f32,
        tr_cy: f32,
        source_cx: f32,
        source_cy: f32,
        scale_type: c_int,
        scale_x: *mut f32,
        scale_y: *mut f32,
    ) -> bool;
    /// `obs_source_transition_clock_mix_spaces`.
    pub fn oracle_obs_source_transition_clock_mix_spaces(a: c_int, b: c_int) -> c_int;
    /// `obs_source_transition_clock_min_ts`.
    pub fn oracle_obs_source_transition_clock_min_ts(a: u64, b: u64) -> u64;
    /// The entry named `LAYOUT_NAMES[which]`; 0 out of range.
    pub fn oracle_source_transition_clock_layout(which: usize) -> usize;
}
