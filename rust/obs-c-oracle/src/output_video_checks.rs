//! `oracle_*` build of `libobs/obs-output-video-checks.c.h`
//! (oracle/output_video_checks.c): the output video size, conversion and
//! capture checks, renamed.
//!
//! The structs are untyped pointers: callers lay them out themselves, and
//! [`oracle_output_video_checks_layout`] says how the C sees them. The
//! conversion choice comes back as [`ConversionChoice`], whose layout the
//! same entries check.

use core::ffi::{c_int, c_void};

/// Index of each entry in `oracle_output_video_checks_constant`, in the
/// C's order.
pub const CONSTANT_NAMES: [&str; 17] = [
    "OBS_OUTPUT_VIDEO_CHECKS_VIDEO",
    "OBS_OUTPUT_VIDEO_CHECKS_AUDIO",
    "OBS_OUTPUT_VIDEO_CHECKS_ENCODED",
    "OBS_OUTPUT_VIDEO_CHECKS_SERVICE",
    "OBS_OUTPUT_VIDEO_CHECKS_MAX_AUDIO_MIXES",
    "OBS_OUTPUT_VIDEO_CHECKS_MAX_VIDEO_ENCODERS",
    "OBS_OUTPUT_VIDEO_CHECKS_MAX_AUDIO_ENCODERS",
    "OBS_OUTPUT_SIZE_ZERO",
    "OBS_OUTPUT_SIZE_ENCODER",
    "OBS_OUTPUT_SIZE_SCALED",
    "OBS_OUTPUT_SIZE_VIDEO",
    "OBS_OUTPUT_BEGIN_READY",
    "OBS_OUTPUT_BEGIN_REFUSE",
    "OBS_OUTPUT_BEGIN_CHECK",
    "OBS_OUTPUT_CONVERSION_NONE",
    "OBS_OUTPUT_CONVERSION_SET",
    "OBS_OUTPUT_CONVERSION_SCALED",
];

/// Index of each entry in `oracle_output_video_checks_layout`, in the C's
/// order.
pub const LAYOUT_NAMES: [&str; 23] = [
    "sizeof(struct obs_output_capture_presence)",
    "alignof(struct obs_output_capture_presence)",
    "offsetof(struct obs_output_capture_presence, flags)",
    "offsetof(struct obs_output_capture_presence, video_encoders)",
    "offsetof(struct obs_output_capture_presence, audio_encoders)",
    "offsetof(struct obs_output_capture_presence, video)",
    "offsetof(struct obs_output_capture_presence, audio)",
    "offsetof(struct obs_output_capture_presence, service)",
    "sizeof(struct obs_output_conversion_sizes)",
    "alignof(struct obs_output_conversion_sizes)",
    "offsetof(struct obs_output_conversion_sizes, set)",
    "offsetof(struct obs_output_conversion_sizes, set_width)",
    "offsetof(struct obs_output_conversion_sizes, set_height)",
    "offsetof(struct obs_output_conversion_sizes, scaled_width)",
    "offsetof(struct obs_output_conversion_sizes, scaled_height)",
    "offsetof(struct obs_output_conversion_sizes, video_width)",
    "offsetof(struct obs_output_conversion_sizes, video_height)",
    "sizeof(struct obs_output_conversion_choice)",
    "alignof(struct obs_output_conversion_choice)",
    "offsetof(struct obs_output_conversion_choice, kind)",
    "offsetof(struct obs_output_conversion_choice, width)",
    "offsetof(struct obs_output_conversion_choice, height)",
    "sizeof(enum obs_output_size_source)",
];

/// `struct obs_output_conversion_choice`, as the C returns it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConversionChoice {
    pub kind: c_int,
    pub width: u32,
    pub height: u32,
}

unsafe extern "C" {
    pub fn oracle_obs_output_first_mixer(mixer_mask: usize) -> usize;

    /// Returns an `enum obs_output_size_source` value.
    pub fn oracle_obs_output_size_route(encoded: bool, has_encoder: bool, scaled: u32) -> u32;

    /// `presence` points to a `struct obs_output_capture_presence`.
    pub fn oracle_obs_output_capture_ready(presence: *const c_void) -> bool;

    /// Returns an `enum obs_output_begin_action` value.
    pub fn oracle_obs_output_begin_gate(delay_active: bool, active: bool) -> u32;

    /// `sizes` points to a `struct obs_output_conversion_sizes`.
    pub fn oracle_obs_output_conversion_plan(sizes: *const c_void) -> ConversionChoice;

    pub fn oracle_obs_output_congestion_clamp(congestion: f32) -> f32;

    /// The entry named `CONSTANT_NAMES[which]`; -1 out of range.
    pub fn oracle_output_video_checks_constant(which: usize) -> i64;

    /// The entry named `LAYOUT_NAMES[which]`; `usize::MAX` out of range.
    pub fn oracle_output_video_checks_layout(which: usize) -> usize;
}
