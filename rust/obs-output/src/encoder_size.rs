//! Safe core of `libobs/obs-encoder-size.c.h`: the encoder size and info
//! choices of `obs-encoder.c`.
//!
//! `obs-encoder.c` keeps `obs_encoder_valid()`, every warning with the
//! encoder's name, `video_output_get_info()`, `audio_output_get_info()`,
//! `video_output_get_width()` / `_height()`,
//! `audio_output_get_sample_rate()`, the plugin's `get_audio_info` and
//! `get_video_info` between the steps, the load of the active flag and every
//! write to the encoder. It reads the fields, passes plain values, and does
//! what the answer says. These keep no state and call nothing in libobs.
//!
//! - [`audio_defaults`] (`get_audio_info()`, before the plugin's
//!   `get_audio_info`): an unknown format, a 0 sample rate and unknown
//!   speakers each come from the audio mix.
//! - [`cap_override`] (`get_video_info()`, after the plugin's
//!   `get_video_info`): an encoder with [`OBS_ENCODER_CAP_SCALING`] gets the
//!   video's size, whatever the plugin set.
//! - [`scaled_gate`] and [`scaled_match`] (`obs_encoder_set_scaled_size()`):
//!   not a video encoder, active, initialized (refused in that order), then
//!   a size equal to the video's disables scaling, any other is set.
//! - [`scaling_enabled`] (`obs_encoder_scaling_enabled()`): either scaled
//!   axis set.
//! - [`video_size_route`] (`obs_encoder_get_width()` / `_height()`, one
//!   axis) and [`sample_rate_route`] (`obs_encoder_get_sample_rate()`): the
//!   wrong type is refused, no media reads 0, else the stored value when it
//!   is set, else the media's.
//!
//! No input comes from outside OBS (scaled sizes from the frontend and
//! plugins; types, caps and audio info from plugins), but nothing here can
//! panic on any input.
//!
//! No intentional differences.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

/// `OBS_ENCODER_AUDIO` (`enum obs_encoder_type`, `obs-encoder.h`).
pub const OBS_ENCODER_AUDIO: i32 = 0;
/// `OBS_ENCODER_VIDEO`.
pub const OBS_ENCODER_VIDEO: i32 = 1;
/// `OBS_ENCODER_CAP_SCALING` (`obs-encoder.h`).
pub const OBS_ENCODER_CAP_SCALING: u32 = 1 << 5;
/// `AUDIO_FORMAT_UNKNOWN` (`media-io/audio-io.h`).
pub const AUDIO_FORMAT_UNKNOWN: i32 = 0;
/// `SPEAKERS_UNKNOWN` (`media-io/audio-io.h`).
pub const SPEAKERS_UNKNOWN: i32 = 0;

/// The fields of `struct audio_convert_info` (and of the mix's `struct
/// audio_output_info`) that `get_audio_info()` fills, the enums as `int`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AudioFill {
    /// `enum audio_format`.
    pub format: i32,
    /// `samples_per_sec`.
    pub samples_per_sec: u32,
    /// `enum speaker_layout`.
    pub speakers: i32,
}

/// A width and a height.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Size {
    /// The width.
    pub width: u32,
    /// The height.
    pub height: u32,
}

/// Where `obs_encoder_get_width()` / `_height()` take one axis from, and
/// `obs_encoder_get_sample_rate()` the rate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizeRoute {
    /// Not a video encoder (not an audio one, for the rate): a warning, 0.
    WrongType,
    /// No video (audio): 0.
    NoMedia,
    /// The scaled size on that axis (the stored sample rate).
    Stored,
    /// `video_output_get_width()` / `_height()`
    /// (`audio_output_get_sample_rate()`) of the encoder's media.
    Media,
}

/// The gates of `obs_encoder_set_scaled_size()`, in the C's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaledGate {
    /// Not a video encoder: a warning, nothing set.
    NotVideo,
    /// The encoder is active: a warning, nothing set.
    Active,
    /// The encoder is initialized: a warning, nothing set.
    Initialized,
    /// Read the video's info, then [`scaled_match`].
    Check,
}

/// What `obs_encoder_set_scaled_size()` does once the gates pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaledMatch {
    /// The size equals the video's: a warning, the scaled size set to
    /// 0 x 0 (scaling off).
    Disable,
    /// The scaled size set to the size asked for.
    Set,
}

/// `get_audio_info()` before the plugin's `get_audio_info`: `info` with an
/// unknown format ([`AUDIO_FORMAT_UNKNOWN`]), a 0 sample rate and unknown
/// speakers ([`SPEAKERS_UNKNOWN`]) each taken from `mix`, the other fields
/// kept. Any other value counts as known, values outside the enums too.
#[must_use]
pub fn audio_defaults(info: AudioFill, mix: AudioFill) -> AudioFill {
    AudioFill {
        format: if info.format == AUDIO_FORMAT_UNKNOWN {
            mix.format
        } else {
            info.format
        },
        samples_per_sec: if info.samples_per_sec == 0 {
            mix.samples_per_sec
        } else {
            info.samples_per_sec
        },
        speakers: if info.speakers == SPEAKERS_UNKNOWN {
            mix.speakers
        } else {
            info.speakers
        },
    }
}

/// `get_video_info()` after the plugin's `get_video_info`: the video's size
/// (`voi`) when `caps` has [`OBS_ENCODER_CAP_SCALING`], else the size the
/// plugin left (`info`).
#[must_use]
pub fn cap_override(caps: u32, info: Size, voi: Size) -> Size {
    // Prevent video output from performing an actual scale. If GPU scaling
    // is enabled, the voi holds the scaled size, so GPU scaling takes
    // priority over the encoder's own scaling.
    if caps & OBS_ENCODER_CAP_SCALING != 0 {
        voi
    } else {
        info
    }
}

/// `obs_encoder_set_scaled_size()` after `obs_encoder_valid()`:
/// `encoder_type` is `info.type`, `active` the loaded active flag,
/// `initialized` the field.
#[must_use]
pub fn scaled_gate(encoder_type: i32, active: bool, initialized: bool) -> ScaledGate {
    if encoder_type != OBS_ENCODER_VIDEO {
        ScaledGate::NotVideo
    } else if active {
        ScaledGate::Active
    } else if initialized {
        ScaledGate::Initialized
    } else {
        ScaledGate::Check
    }
}

/// `obs_encoder_set_scaled_size()` after the gates: `voi` is the video's
/// size (`None` when `video_output_get_info()` is NULL), `size` the size
/// asked for. [`ScaledMatch::Disable`] when they are equal on both axes.
#[must_use]
pub fn scaled_match(voi: Option<Size>, size: Size) -> ScaledMatch {
    if voi == Some(size) {
        ScaledMatch::Disable
    } else {
        ScaledMatch::Set
    }
}

/// `obs_encoder_scaling_enabled()` after `obs_encoder_valid()`: either
/// scaled axis is not 0.
#[must_use]
pub fn scaling_enabled(scaled: Size) -> bool {
    scaled.width != 0 || scaled.height != 0
}

/// One axis of `obs_encoder_get_width()` / `_height()` after
/// `obs_encoder_valid()`: `encoder_type` is `info.type`, `has_media`
/// whether the encoder has a video, `scaled` the scaled size on that axis.
#[must_use]
pub fn video_size_route(encoder_type: i32, has_media: bool, scaled: u32) -> SizeRoute {
    route(encoder_type == OBS_ENCODER_VIDEO, has_media, scaled)
}

/// `obs_encoder_get_sample_rate()` after `obs_encoder_valid()`:
/// `encoder_type` is `info.type`, `has_media` whether the encoder has an
/// audio, `samplerate` the rate stored at initialization.
#[must_use]
pub fn sample_rate_route(encoder_type: i32, has_media: bool, samplerate: u32) -> SizeRoute {
    route(encoder_type == OBS_ENCODER_AUDIO, has_media, samplerate)
}

/// Both getters: the type, then the media, then the stored value when it
/// is set.
fn route(right_type: bool, has_media: bool, stored: u32) -> SizeRoute {
    if !right_type {
        SizeRoute::WrongType
    } else if !has_media {
        SizeRoute::NoMedia
    } else if stored != 0 {
        SizeRoute::Stored
    } else {
        SizeRoute::Media
    }
}
