//! `oracle_*` build of `libobs/media-io/audio-resampler-plan.c.h`
//! (oracle/audio_resampler_plan.c).
//!
//! `enum speaker_layout` and `enum audio_format` cross as `int`.

use core::ffi::c_int;

/// Index of each value in `oracle_audio_resampler_plan_int`, in the C's
/// order.
pub const INT_NAMES: [&str; 7] = [
    "AUDIO_RESAMPLER_CH_LAYOUT_MONO",
    "AUDIO_RESAMPLER_CH_LAYOUT_STEREO",
    "AUDIO_RESAMPLER_CH_LAYOUT_SURROUND",
    "AUDIO_RESAMPLER_CH_LAYOUT_4POINT0",
    "AUDIO_RESAMPLER_CH_LAYOUT_4POINT1",
    "AUDIO_RESAMPLER_CH_LAYOUT_5POINT1_BACK",
    "AUDIO_RESAMPLER_CH_LAYOUT_7POINT1",
];

unsafe extern "C" {
    pub fn oracle_audio_resampler_layout_mask(layout: c_int) -> u64;
    pub fn oracle_audio_resampler_output_channels(dst: c_int) -> u32;
    pub fn oracle_audio_resampler_planar_output(dst_format: c_int) -> bool;
    pub fn oracle_audio_resampler_input_channels(src: c_int) -> c_int;
    pub fn oracle_audio_resampler_is_4point1(layout: c_int) -> bool;
    pub fn oracle_audio_resampler_upmixes_mono(output_ch: u32) -> bool;
    /// The value at `which` in [`INT_NAMES`], or -1 past the end.
    pub fn oracle_audio_resampler_plan_int(which: usize) -> i64;
}
