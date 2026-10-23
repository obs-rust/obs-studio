//! `oracle_*` build of `libobs/obs-source-audio-input.c.h`
//! (oracle/source_audio_input.c): the source audio input steps, renamed.
//! The format is passed by value in the C layout, mirrored here.

use core::ffi::c_int;

/// Index of each entry in `oracle_source_audio_input_layout`, in the C's
/// order.
pub const LAYOUT_NAMES: [&str; 13] = [
    "sizeof(struct obs_source_audio_input_format)",
    "alignof(struct obs_source_audio_input_format)",
    "offsetof(format, format)",
    "offsetof(format, samples_per_sec)",
    "offsetof(format, speakers)",
    "OBS_SOURCE_AUDIO_INPUT_SMOOTHING_THRESHOLD",
    "OBS_SOURCE_AUDIO_INPUT_SPEAKERS_STEREO",
    "OBS_SOURCE_AUDIO_INPUT_FORCE_MONO",
    "OBS_SOURCE_AUDIO_INPUT_SINE_LAW",
    "OBS_SOURCE_AUDIO_INPUT_SQUARE_LAW",
    "OBS_SOURCE_AUDIO_INPUT_LINEAR",
    "sizeof(int)",
    "sizeof(bool)",
];

/// `struct obs_source_audio_input_format`, as the C lays it out.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleAudioInputFormat {
    pub format: c_int,
    pub samples_per_sec: u32,
    pub speakers: c_int,
}

unsafe extern "C" {
    pub fn oracle_obs_source_audio_input_discontinuous(
        has_resampler: bool,
        next_resample_in_ts: u64,
        timestamp: u64,
    ) -> bool;
    pub fn oracle_obs_source_audio_input_reset_needed(
        current: OracleAudioInputFormat,
        packet: OracleAudioInputFormat,
        last_compensation: bool,
        compensation: bool,
        discontinuous: bool,
    ) -> bool;
    pub fn oracle_obs_source_audio_input_resampler_needed(
        source: OracleAudioInputFormat,
        output: OracleAudioInputFormat,
        compensation: bool,
    ) -> bool;
    pub fn oracle_obs_source_audio_input_balance_needed(
        mono_output: bool,
        speakers: c_int,
        balance: f32,
    ) -> bool;
    pub fn oracle_obs_source_audio_input_mono_needed(mono_output: bool, flags: u32) -> bool;
    /// `data` holds two plane pointers of `frames` floats each, for a known
    /// law and `frames > 0`.
    pub fn oracle_obs_source_audio_input_balance(
        data: *mut *mut f32,
        frames: u32,
        balance: f32,
        law: c_int,
    );
    /// `data` holds `channels` plane pointers (one for 0) of `frames`
    /// floats each, for `frames > 0`.
    pub fn oracle_obs_source_audio_input_downmix(data: *mut *mut f32, channels: usize, frames: u32);
    /// The entry named `LAYOUT_NAMES[which]`; 0 out of range.
    pub fn oracle_source_audio_input_layout(which: usize) -> usize;
}
