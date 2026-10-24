//! C ABI for `libobs/obs-output-video-checks.c.h`, the output video size,
//! conversion and capture checks of `obs-output.c`, declared in
//! `libobs/obs-output-video-checks.h` without `EXPORT`.
//!
//! `obs-output.c.h` (still C) checks the output (`obs_output_valid()`),
//! logs the flag warnings, joins `end_data_capture_thread`, calls
//! `obs_encoder_get_width()` / `_height()`, `video_output_get_*()` and
//! `info.get_congestion`, loads the atomics, and passes the results here by
//! value, the presence and the sizes through
//! [`obs_output_capture_presence`] and [`obs_output_conversion_sizes`]
//! (Tier 3 checks their layout against the C). It fills `video_conversion`
//! from the plan. So `struct obs_output` is never mirrored, and these shims
//! take no lock and call nothing in libobs. All stay hidden
//! (`rust-hidden.txt`).
//!
//! No intentional differences. A null pointer is not checked: libobs passes
//! a local.

use std::os::raw::c_int;

use crate::video_checks::{
    self as core, BeginGate, Conversion, ConversionSizes, Presence, SizeRoute,
};

/// `struct obs_output_capture_presence` (`obs-output-video-checks.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_output_capture_presence {
    pub flags: u32,
    pub video_encoders: u32,
    pub audio_encoders: u32,
    pub video: bool,
    pub audio: bool,
    pub service: bool,
}

/// `struct obs_output_conversion_sizes` (`obs-output-video-checks.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_output_conversion_sizes {
    pub set: bool,
    pub set_width: u32,
    pub set_height: u32,
    pub scaled_width: u32,
    pub scaled_height: u32,
    pub video_width: u32,
    pub video_height: u32,
}

/// `struct obs_output_conversion_choice` (`obs-output-video-checks.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_output_conversion_choice {
    /// An `enum obs_output_conversion_kind` value.
    pub kind: c_int,
    pub width: u32,
    pub height: u32,
}

/// `enum obs_output_size_source` and `enum obs_output_begin_action`, as the
/// C's enums are returned (an `int` or `unsigned int`; every value fits
/// both).
#[allow(non_camel_case_types)]
pub type obs_output_video_checks_enum = u32;

/// `OBS_OUTPUT_SIZE_ZERO`.
pub const OBS_OUTPUT_SIZE_ZERO: obs_output_video_checks_enum = 0;
/// `OBS_OUTPUT_SIZE_ENCODER`.
pub const OBS_OUTPUT_SIZE_ENCODER: obs_output_video_checks_enum = 1;
/// `OBS_OUTPUT_SIZE_SCALED`.
pub const OBS_OUTPUT_SIZE_SCALED: obs_output_video_checks_enum = 2;
/// `OBS_OUTPUT_SIZE_VIDEO`.
pub const OBS_OUTPUT_SIZE_VIDEO: obs_output_video_checks_enum = 3;

/// `OBS_OUTPUT_BEGIN_READY`.
pub const OBS_OUTPUT_BEGIN_READY: obs_output_video_checks_enum = 0;
/// `OBS_OUTPUT_BEGIN_REFUSE`.
pub const OBS_OUTPUT_BEGIN_REFUSE: obs_output_video_checks_enum = 1;
/// `OBS_OUTPUT_BEGIN_CHECK`.
pub const OBS_OUTPUT_BEGIN_CHECK: obs_output_video_checks_enum = 2;

/// `OBS_OUTPUT_CONVERSION_NONE`.
pub const OBS_OUTPUT_CONVERSION_NONE: c_int = 0;
/// `OBS_OUTPUT_CONVERSION_SET`.
pub const OBS_OUTPUT_CONVERSION_SET: c_int = 1;
/// `OBS_OUTPUT_CONVERSION_SCALED`.
pub const OBS_OUTPUT_CONVERSION_SCALED: c_int = 2;

/// `size_t obs_output_first_mixer(size_t mixer_mask)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_first_mixer(mixer_mask: usize) -> usize {
    core::first_mixer(mixer_mask)
}

/// `enum obs_output_size_source obs_output_size_route(bool encoded, bool
/// has_encoder, uint32_t scaled)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_size_route(
    encoded: bool,
    has_encoder: bool,
    scaled: u32,
) -> obs_output_video_checks_enum {
    match core::size_route(encoded, has_encoder, scaled) {
        SizeRoute::Zero => OBS_OUTPUT_SIZE_ZERO,
        SizeRoute::Encoder => OBS_OUTPUT_SIZE_ENCODER,
        SizeRoute::Scaled => OBS_OUTPUT_SIZE_SCALED,
        SizeRoute::Video => OBS_OUTPUT_SIZE_VIDEO,
    }
}

/// `bool obs_output_capture_ready(const struct obs_output_capture_presence
/// *presence)`.
///
/// # Safety
///
/// As the C: `presence` points to a readable `struct
/// obs_output_capture_presence`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_capture_ready(
    presence: *const obs_output_capture_presence,
) -> bool {
    // SAFETY: a readable presence (caller contract). Copied, not borrowed.
    let p = unsafe { presence.read() };
    core::capture_ready(&Presence {
        flags: p.flags,
        video_encoders: p.video_encoders,
        audio_encoders: p.audio_encoders,
        video: p.video,
        audio: p.audio,
        service: p.service,
    })
}

/// `enum obs_output_begin_action obs_output_begin_gate(bool delay_active,
/// bool active)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_begin_gate(
    delay_active: bool,
    active: bool,
) -> obs_output_video_checks_enum {
    match core::begin_gate(delay_active, active) {
        BeginGate::Ready => OBS_OUTPUT_BEGIN_READY,
        BeginGate::Refuse => OBS_OUTPUT_BEGIN_REFUSE,
        BeginGate::Check => OBS_OUTPUT_BEGIN_CHECK,
    }
}

/// `struct obs_output_conversion_choice obs_output_conversion_plan(const
/// struct obs_output_conversion_sizes *sizes)`. Width and height are 0
/// with `OBS_OUTPUT_CONVERSION_NONE`, as the C's initializer.
///
/// # Safety
///
/// As the C: `sizes` points to a readable `struct
/// obs_output_conversion_sizes`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_conversion_plan(
    sizes: *const obs_output_conversion_sizes,
) -> obs_output_conversion_choice {
    // SAFETY: readable sizes (caller contract). Copied, not borrowed.
    let s = unsafe { sizes.read() };
    let plan = core::video_conversion(&ConversionSizes {
        set: s.set,
        set_width: s.set_width,
        set_height: s.set_height,
        scaled_width: s.scaled_width,
        scaled_height: s.scaled_height,
        video_width: s.video_width,
        video_height: s.video_height,
    });
    match plan {
        Conversion::None => obs_output_conversion_choice {
            kind: OBS_OUTPUT_CONVERSION_NONE,
            width: 0,
            height: 0,
        },
        Conversion::UseSet { width, height } => obs_output_conversion_choice {
            kind: OBS_OUTPUT_CONVERSION_SET,
            width,
            height,
        },
        Conversion::Scaled { width, height } => obs_output_conversion_choice {
            kind: OBS_OUTPUT_CONVERSION_SCALED,
            width,
            height,
        },
    }
}

/// `float obs_output_congestion_clamp(float congestion)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_congestion_clamp(congestion: f32) -> f32 {
    core::congestion(congestion)
}
