//! C ABI for `libobs/obs-encoder-size.c.h`, the encoder size and info
//! choices of `obs-encoder.c`, declared in `libobs/obs-encoder-size.h`
//! without `EXPORT`.
//!
//! `obs-encoder.c.h` (still C) checks the encoder (`obs_encoder_valid()`),
//! logs every warning with the encoder's name, calls
//! `video_output_get_info()`, `audio_output_get_info()`,
//! `video_output_get_width()` / `_height()`, `audio_output_get_sample_rate()`
//! and the plugin's `get_audio_info` / `get_video_info` between the steps,
//! loads the active flag, and passes the results here by value, the audio
//! fields through [`obs_encoder_audio_fill`] (Tier 3 checks its layout
//! against the C). It writes the answers back into the encoder and the
//! infos. So `struct obs_encoder` is never mirrored, and these shims take
//! no lock and call nothing in libobs. All stay hidden (`rust-hidden.txt`).
//!
//! No intentional differences. A null pointer is not checked: libobs passes
//! locals.

use std::os::raw::c_int;

use crate::encoder_size::{self as core, AudioFill, ScaledGate, ScaledMatch, Size, SizeRoute};

/// `struct obs_encoder_audio_fill` (`obs-encoder-size.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_encoder_audio_fill {
    /// An `enum audio_format` value.
    pub format: c_int,
    pub samples_per_sec: u32,
    /// An `enum speaker_layout` value.
    pub speakers: c_int,
}

/// `struct obs_encoder_size` (`obs-encoder-size.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_encoder_size {
    pub width: u32,
    pub height: u32,
}

/// `enum obs_encoder_size_source` and `enum obs_encoder_scaled_action`, as
/// the C's enums are returned (an `int` or `unsigned int`; every value fits
/// both).
#[allow(non_camel_case_types)]
pub type obs_encoder_size_enum = u32;

/// `OBS_ENCODER_SIZE_WRONG_TYPE`.
pub const OBS_ENCODER_SIZE_WRONG_TYPE: obs_encoder_size_enum = 0;
/// `OBS_ENCODER_SIZE_NO_MEDIA`.
pub const OBS_ENCODER_SIZE_NO_MEDIA: obs_encoder_size_enum = 1;
/// `OBS_ENCODER_SIZE_STORED`.
pub const OBS_ENCODER_SIZE_STORED: obs_encoder_size_enum = 2;
/// `OBS_ENCODER_SIZE_MEDIA`.
pub const OBS_ENCODER_SIZE_MEDIA: obs_encoder_size_enum = 3;

/// `OBS_ENCODER_SCALED_NOT_VIDEO`.
pub const OBS_ENCODER_SCALED_NOT_VIDEO: obs_encoder_size_enum = 0;
/// `OBS_ENCODER_SCALED_ACTIVE`.
pub const OBS_ENCODER_SCALED_ACTIVE: obs_encoder_size_enum = 1;
/// `OBS_ENCODER_SCALED_INITIALIZED`.
pub const OBS_ENCODER_SCALED_INITIALIZED: obs_encoder_size_enum = 2;
/// `OBS_ENCODER_SCALED_CHECK`.
pub const OBS_ENCODER_SCALED_CHECK: obs_encoder_size_enum = 3;
/// `OBS_ENCODER_SCALED_DISABLE`.
pub const OBS_ENCODER_SCALED_DISABLE: obs_encoder_size_enum = 4;
/// `OBS_ENCODER_SCALED_SET`.
pub const OBS_ENCODER_SCALED_SET: obs_encoder_size_enum = 5;

fn route_value(route: SizeRoute) -> obs_encoder_size_enum {
    match route {
        SizeRoute::WrongType => OBS_ENCODER_SIZE_WRONG_TYPE,
        SizeRoute::NoMedia => OBS_ENCODER_SIZE_NO_MEDIA,
        SizeRoute::Stored => OBS_ENCODER_SIZE_STORED,
        SizeRoute::Media => OBS_ENCODER_SIZE_MEDIA,
    }
}

fn fill(f: obs_encoder_audio_fill) -> AudioFill {
    AudioFill {
        format: f.format,
        samples_per_sec: f.samples_per_sec,
        speakers: f.speakers,
    }
}

/// `struct obs_encoder_audio_fill obs_encoder_size_audio_defaults(const
/// struct obs_encoder_audio_fill *info, const struct obs_encoder_audio_fill
/// *mix)`.
///
/// # Safety
///
/// As the C: `info` and `mix` point to readable `struct
/// obs_encoder_audio_fill`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_size_audio_defaults(
    info: *const obs_encoder_audio_fill,
    mix: *const obs_encoder_audio_fill,
) -> obs_encoder_audio_fill {
    // SAFETY: readable fills (caller contract). Copied, not borrowed.
    let (info, mix) = unsafe { (info.read(), mix.read()) };
    let out = core::audio_defaults(fill(info), fill(mix));
    obs_encoder_audio_fill {
        format: out.format,
        samples_per_sec: out.samples_per_sec,
        speakers: out.speakers,
    }
}

/// `struct obs_encoder_size obs_encoder_size_cap_override(uint32_t caps,
/// uint32_t width, uint32_t height, uint32_t voi_width, uint32_t
/// voi_height)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_size_cap_override(
    caps: u32,
    width: u32,
    height: u32,
    voi_width: u32,
    voi_height: u32,
) -> obs_encoder_size {
    let size = core::cap_override(
        caps,
        Size { width, height },
        Size {
            width: voi_width,
            height: voi_height,
        },
    );
    obs_encoder_size {
        width: size.width,
        height: size.height,
    }
}

/// `enum obs_encoder_scaled_action obs_encoder_size_scaled_gate(int type,
/// bool active, bool initialized)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_size_scaled_gate(
    encoder_type: c_int,
    active: bool,
    initialized: bool,
) -> obs_encoder_size_enum {
    match core::scaled_gate(encoder_type, active, initialized) {
        ScaledGate::NotVideo => OBS_ENCODER_SCALED_NOT_VIDEO,
        ScaledGate::Active => OBS_ENCODER_SCALED_ACTIVE,
        ScaledGate::Initialized => OBS_ENCODER_SCALED_INITIALIZED,
        ScaledGate::Check => OBS_ENCODER_SCALED_CHECK,
    }
}

/// `enum obs_encoder_scaled_action obs_encoder_size_scaled_match(bool
/// has_voi, uint32_t voi_width, uint32_t voi_height, uint32_t width,
/// uint32_t height)`. The voi's size is not read without `has_voi`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_size_scaled_match(
    has_voi: bool,
    voi_width: u32,
    voi_height: u32,
    width: u32,
    height: u32,
) -> obs_encoder_size_enum {
    let voi = has_voi.then_some(Size {
        width: voi_width,
        height: voi_height,
    });
    match core::scaled_match(voi, Size { width, height }) {
        ScaledMatch::Disable => OBS_ENCODER_SCALED_DISABLE,
        ScaledMatch::Set => OBS_ENCODER_SCALED_SET,
    }
}

/// `bool obs_encoder_size_scaling(uint32_t scaled_width, uint32_t
/// scaled_height)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_size_scaling(scaled_width: u32, scaled_height: u32) -> bool {
    core::scaling_enabled(Size {
        width: scaled_width,
        height: scaled_height,
    })
}

/// `enum obs_encoder_size_source obs_encoder_size_video_route(int type,
/// bool has_media, uint32_t scaled)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_size_video_route(
    encoder_type: c_int,
    has_media: bool,
    scaled: u32,
) -> obs_encoder_size_enum {
    route_value(core::video_size_route(encoder_type, has_media, scaled))
}

/// `enum obs_encoder_size_source obs_encoder_size_rate_route(int type, bool
/// has_media, uint32_t samplerate)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_size_rate_route(
    encoder_type: c_int,
    has_media: bool,
    samplerate: u32,
) -> obs_encoder_size_enum {
    route_value(core::sample_rate_route(encoder_type, has_media, samplerate))
}
