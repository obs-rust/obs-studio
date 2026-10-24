//! `oracle_*` build of `libobs/obs-encoder-size.c.h`
//! (oracle/encoder_size.c): the encoder size and info choices, renamed.
//!
//! The input struct is an untyped pointer: callers lay it out themselves,
//! and [`oracle_encoder_size_layout`] says how the C sees it. The structs
//! returned by value come back as [`AudioFill`] and [`EncoderSize`], whose
//! layout the same entries check.

use core::ffi::{c_int, c_void};

/// Index of each entry in `oracle_encoder_size_constant`, in the C's order.
pub const CONSTANT_NAMES: [&str; 15] = [
    "OBS_ENCODER_SIZE_TYPE_AUDIO",
    "OBS_ENCODER_SIZE_TYPE_VIDEO",
    "OBS_ENCODER_SIZE_CAP_SCALING",
    "OBS_ENCODER_SIZE_AUDIO_FORMAT_UNKNOWN",
    "OBS_ENCODER_SIZE_SPEAKERS_UNKNOWN",
    "OBS_ENCODER_SIZE_WRONG_TYPE",
    "OBS_ENCODER_SIZE_NO_MEDIA",
    "OBS_ENCODER_SIZE_STORED",
    "OBS_ENCODER_SIZE_MEDIA",
    "OBS_ENCODER_SCALED_NOT_VIDEO",
    "OBS_ENCODER_SCALED_ACTIVE",
    "OBS_ENCODER_SCALED_INITIALIZED",
    "OBS_ENCODER_SCALED_CHECK",
    "OBS_ENCODER_SCALED_DISABLE",
    "OBS_ENCODER_SCALED_SET",
];

/// Index of each entry in `oracle_encoder_size_layout`, in the C's order.
pub const LAYOUT_NAMES: [&str; 11] = [
    "sizeof(struct obs_encoder_audio_fill)",
    "alignof(struct obs_encoder_audio_fill)",
    "offsetof(struct obs_encoder_audio_fill, format)",
    "offsetof(struct obs_encoder_audio_fill, samples_per_sec)",
    "offsetof(struct obs_encoder_audio_fill, speakers)",
    "sizeof(struct obs_encoder_size)",
    "alignof(struct obs_encoder_size)",
    "offsetof(struct obs_encoder_size, width)",
    "offsetof(struct obs_encoder_size, height)",
    "sizeof(enum obs_encoder_size_source)",
    "sizeof(enum obs_encoder_scaled_action)",
];

/// `struct obs_encoder_audio_fill`, as the C returns it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AudioFill {
    pub format: c_int,
    pub samples_per_sec: u32,
    pub speakers: c_int,
}

/// `struct obs_encoder_size`, as the C returns it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EncoderSize {
    pub width: u32,
    pub height: u32,
}

unsafe extern "C" {
    /// `info` and `mix` point to `struct obs_encoder_audio_fill`s.
    pub fn oracle_obs_encoder_size_audio_defaults(
        info: *const c_void,
        mix: *const c_void,
    ) -> AudioFill;

    pub fn oracle_obs_encoder_size_cap_override(
        caps: u32,
        width: u32,
        height: u32,
        voi_width: u32,
        voi_height: u32,
    ) -> EncoderSize;

    /// Returns an `enum obs_encoder_scaled_action` value.
    pub fn oracle_obs_encoder_size_scaled_gate(
        encoder_type: c_int,
        active: bool,
        initialized: bool,
    ) -> u32;

    /// Returns an `enum obs_encoder_scaled_action` value.
    pub fn oracle_obs_encoder_size_scaled_match(
        has_voi: bool,
        voi_width: u32,
        voi_height: u32,
        width: u32,
        height: u32,
    ) -> u32;

    pub fn oracle_obs_encoder_size_scaling(scaled_width: u32, scaled_height: u32) -> bool;

    /// Returns an `enum obs_encoder_size_source` value.
    pub fn oracle_obs_encoder_size_video_route(
        encoder_type: c_int,
        has_media: bool,
        scaled: u32,
    ) -> u32;

    /// Returns an `enum obs_encoder_size_source` value.
    pub fn oracle_obs_encoder_size_rate_route(
        encoder_type: c_int,
        has_media: bool,
        samplerate: u32,
    ) -> u32;

    /// The entry named `CONSTANT_NAMES[which]`; -1 out of range.
    pub fn oracle_encoder_size_constant(which: usize) -> i64;

    /// The entry named `LAYOUT_NAMES[which]`; `usize::MAX` out of range.
    pub fn oracle_encoder_size_layout(which: usize) -> usize;
}
