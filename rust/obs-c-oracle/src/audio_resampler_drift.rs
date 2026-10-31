//! `oracle_*` build of `libobs/media-io/audio-resampler-drift.c.h`
//! (oracle/audio_resampler_drift.c).
//!
//! `enum audio_format` crosses as `int`, `enum audio_resampler_comp_action`
//! (0 to 2) as `unsigned int`, and `struct audio_resampler_comp` as
//! [`Comp`], laid out as the C (checked against the C's `offsetof()` by the
//! parity test).

use core::ffi::{c_int, c_uint};

/// `struct audio_resampler_comp`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Comp {
    pub enabled: bool,
    pub engaged: bool,
    pub error_ns: i64,
    pub vc1: f64,
    pub vc2: f64,
    pub input_samples: u64,
    pub output_samples: u64,
}

/// Index of each value in `oracle_audio_resampler_drift_int`, in the C's
/// order.
pub const INT_NAMES: [&str; 24] = [
    "AUDIO_RESAMPLER_COMP_DEAD_BAND_NS",
    "AUDIO_RESAMPLER_COMP_DISTANCE",
    "AUDIO_RESAMPLER_FMT_U8",
    "AUDIO_RESAMPLER_FMT_S16",
    "AUDIO_RESAMPLER_FMT_S32",
    "AUDIO_RESAMPLER_FMT_FLT",
    "AUDIO_RESAMPLER_FMT_U8P",
    "AUDIO_RESAMPLER_FMT_S16P",
    "AUDIO_RESAMPLER_FMT_S32P",
    "AUDIO_RESAMPLER_FMT_FLTP",
    "MAX_AUDIO_CHANNELS",
    "AUDIO_RESAMPLER_COMP_IDLE",
    "AUDIO_RESAMPLER_COMP_ENGAGE",
    "AUDIO_RESAMPLER_COMP_SET",
    "sizeof(enum audio_resampler_comp_action)",
    "sizeof(struct audio_resampler_comp)",
    "_Alignof(struct audio_resampler_comp)",
    "offsetof(enabled)",
    "offsetof(engaged)",
    "offsetof(error_ns)",
    "offsetof(vc1)",
    "offsetof(vc2)",
    "offsetof(input_samples)",
    "offsetof(output_samples)",
];

/// Index of each value in `oracle_audio_resampler_drift_double`.
pub const DOUBLE_NAMES: [&str; 4] = [
    "AUDIO_RESAMPLER_COMP_MAX_PPM",
    "AUDIO_RESAMPLER_COMP_GAIN",
    "AUDIO_RESAMPLER_COMP_C1",
    "AUDIO_RESAMPLER_COMP_C2",
];

unsafe extern "C" {
    pub fn oracle_audio_resampler_sample_format(format: c_int) -> c_int;
    pub fn oracle_audio_resampler_upmix_row(output_ch: u32, row: *mut f64) -> bool;
    pub fn oracle_audio_resampler_comp_set_error(c: *mut Comp, error_ns: i64);
    pub fn oracle_audio_resampler_comp_step(
        c: *mut Comp,
        input_freq: u32,
        in_frames: u32,
    ) -> c_uint;
    pub fn oracle_audio_resampler_comp_delta(c: *const Comp, input_freq: u32) -> c_int;
    pub fn oracle_audio_resampler_output_frames(estimated: c_int, engaged: bool) -> c_int;
    pub fn oracle_audio_resampler_comp_count(c: *mut Comp, in_frames: u32, converted: c_int);
    pub fn oracle_audio_resampler_comp_reports_drift(c: *const Comp) -> bool;
    pub fn oracle_audio_resampler_comp_input_rate(c: *const Comp, output_freq: u32) -> f64;
    /// The value at `which` in [`INT_NAMES`], or -1 past the end.
    pub fn oracle_audio_resampler_drift_int(which: usize) -> i64;
    /// The value at `which` in [`DOUBLE_NAMES`], or -1 past the end.
    pub fn oracle_audio_resampler_drift_double(which: usize) -> f64;
}
