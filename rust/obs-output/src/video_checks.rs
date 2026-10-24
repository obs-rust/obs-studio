//! Safe core of `libobs/obs-output-video-checks.c.h`: the output video size,
//! conversion and capture checks of `obs-output.c`.
//!
//! `obs-output.c` keeps `obs_output_valid()`, the `log_flag_*` warnings, the
//! `pthread_join()` of `end_data_capture_thread`, `obs_encoder_get_width()`
//! and `_height()`, `video_output_get_*()` and `info.get_congestion`. It
//! loads the atomics and reads the fields, passes plain values, and does
//! what the answer says. These keep no state and call nothing in libobs.
//!
//! - [`first_mixer`] (`get_first_mixer()`): the lowest set bit of the mixer
//!   mask below [`MAX_AUDIO_MIXES`], else 0.
//! - [`size_route`] (`obs_output_get_width2()` / `_height2()`, one axis):
//!   an encoded output asks its encoder (0 without one); a raw output takes
//!   the scaled size when it is set, else the video's.
//! - [`capture_ready`] (`can_begin_data_capture()`): each wanted part
//!   (video, audio, service) must be there; video and audio are the
//!   encoders of an encoded output, the video and audio of a raw one.
//! - [`begin_gate`] (`obs_output_can_begin_data_capture()`): a delayed
//!   output is ready, an active one is refused, the rest go on to the join
//!   and [`capture_ready`].
//! - [`video_conversion`] (`obs_output_get_video_conversion()`): the set
//!   conversion with a 0 width or height filled from the output's size, a
//!   conversion to the scaled size when it differs from the video's, or
//!   none.
//! - [`congestion`] (`obs_output_get_congestion()`): below 0 is 0, above 1
//!   is 1. A NaN passes through, as in C ("characterized, not endorsed").
//!
//! No input comes from outside OBS (sizes, flags and mixers come from the
//! frontend and plugins, the congestion from the output plugin), but
//! nothing here can panic on any input.
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

/// `OBS_OUTPUT_VIDEO` (`obs-output.h`).
pub const OBS_OUTPUT_VIDEO: u32 = 1 << 0;
/// `OBS_OUTPUT_AUDIO`.
pub const OBS_OUTPUT_AUDIO: u32 = 1 << 1;
/// `OBS_OUTPUT_ENCODED`.
pub const OBS_OUTPUT_ENCODED: u32 = 1 << 2;
/// `OBS_OUTPUT_SERVICE`.
pub const OBS_OUTPUT_SERVICE: u32 = 1 << 3;

/// `MAX_AUDIO_MIXES` (`media-io/audio-io.h`).
pub const MAX_AUDIO_MIXES: usize = 6;
/// `MAX_OUTPUT_VIDEO_ENCODERS` (`obs-output.h`).
pub const MAX_OUTPUT_VIDEO_ENCODERS: u32 = 10;
/// `MAX_OUTPUT_AUDIO_ENCODERS` (`obs-output.h`).
pub const MAX_OUTPUT_AUDIO_ENCODERS: u32 = 6;

/// Where `obs_output_get_width2()` / `_height2()` take one axis from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizeRoute {
    /// An encoded output without an encoder at the index: 0.
    Zero,
    /// `obs_encoder_get_width()` / `_height()` of that encoder.
    Encoder,
    /// The scaled size.
    Scaled,
    /// `video_output_get_width()` / `_height()` of the output's video.
    Video,
}

/// What `obs_output_can_begin_data_capture()` does before the join.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeginGate {
    /// The delay is active: true.
    Ready,
    /// The output is active: false.
    Refuse,
    /// Join a data capture that is ending, then [`capture_ready`].
    Check,
}

/// What `can_begin_data_capture()` looks at.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Presence {
    /// `info.flags`.
    pub flags: u32,
    /// Bit `i` set: `video_encoders[i]` is set.
    pub video_encoders: u32,
    /// Bit `i` set: `audio_encoders[i]` is set.
    pub audio_encoders: u32,
    /// `output->video != NULL`.
    pub video: bool,
    /// `output->audio != NULL`.
    pub audio: bool,
    /// `output->service != NULL`.
    pub service: bool,
}

/// What `obs_output_get_video_conversion()` looks at, once it has checked
/// that the output is a raw video output.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConversionSizes {
    /// `video_conversion_set`.
    pub set: bool,
    /// `video_conversion.width`.
    pub set_width: u32,
    /// `video_conversion.height`.
    pub set_height: u32,
    /// `scaled_width`.
    pub scaled_width: u32,
    /// `scaled_height`.
    pub scaled_height: u32,
    /// `video_output_get_width()` of the output's video (0 without one).
    pub video_width: u32,
    /// `video_output_get_height()`.
    pub video_height: u32,
}

/// What `obs_output_get_video_conversion()` returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conversion {
    /// NULL.
    None,
    /// The set `video_conversion`, with this width and height.
    UseSet { width: u32, height: u32 },
    /// `video_conversion` built from the video's format, the default colour
    /// space and range, and this (the scaled) size. The C caller returns NULL
    /// instead when the output has no video, as there is no format (#2127).
    Scaled { width: u32, height: u32 },
}

/// `get_first_mixer()`: the lowest `i` below [`MAX_AUDIO_MIXES`] whose bit
/// is set in `mixer_mask`, or 0 when none is (also for a mask with bits only
/// at or above [`MAX_AUDIO_MIXES`]).
#[must_use]
pub fn first_mixer(mixer_mask: usize) -> usize {
    (0..MAX_AUDIO_MIXES)
        .find(|&i| mixer_mask & (1usize << i) != 0)
        .unwrap_or(0)
}

/// One axis of `obs_output_get_width2()` / `_height2()`, after the
/// validity, flag and index checks: `encoded` is `OBS_OUTPUT_ENCODED`,
/// `has_encoder` whether the encoder at the index is set, `scaled` the
/// scaled size on that axis.
#[must_use]
pub fn size_route(encoded: bool, has_encoder: bool, scaled: u32) -> SizeRoute {
    if encoded {
        if has_encoder {
            SizeRoute::Encoder
        } else {
            SizeRoute::Zero
        }
    } else if scaled != 0 {
        SizeRoute::Scaled
    } else {
        SizeRoute::Video
    }
}

/// Whether `mask` has a bit set below `limit` (the C's loop over the
/// encoder array).
fn any_below(mask: u32, limit: u32) -> bool {
    (0..limit).any(|i| mask & (1u32 << i) != 0)
}

/// `can_begin_data_capture()`: false when the output wants video
/// ([`OBS_OUTPUT_VIDEO`]) without a video encoder (encoded) or a video
/// (raw), audio without an audio encoder or audio, or a service without
/// one; true otherwise. Encoder bits at or above the limits are ignored.
#[must_use]
pub fn capture_ready(p: &Presence) -> bool {
    let encoded = p.flags & OBS_OUTPUT_ENCODED != 0;
    let video_valid = if encoded {
        any_below(p.video_encoders, MAX_OUTPUT_VIDEO_ENCODERS)
    } else {
        p.video
    };
    if p.flags & OBS_OUTPUT_VIDEO != 0 && !video_valid {
        return false;
    }
    let audio_valid = if encoded {
        any_below(p.audio_encoders, MAX_OUTPUT_AUDIO_ENCODERS)
    } else {
        p.audio
    };
    if p.flags & OBS_OUTPUT_AUDIO != 0 && !audio_valid {
        return false;
    }
    if p.flags & OBS_OUTPUT_SERVICE != 0 && !p.service {
        return false;
    }
    true
}

/// The gates of `obs_output_can_begin_data_capture()`, on the loaded
/// `delay_active` and `active` atomics.
#[must_use]
pub fn begin_gate(delay_active: bool, active: bool) -> BeginGate {
    if delay_active {
        BeginGate::Ready
    } else if active {
        BeginGate::Refuse
    } else {
        BeginGate::Check
    }
}

/// `obs_output_get_width()` / `_height()` of a raw output on one axis: the
/// scaled size when it is set, else the video's.
fn raw_size(scaled: u32, video: u32) -> u32 {
    if scaled != 0 { scaled } else { video }
}

/// `obs_output_get_video_conversion()` after its flag checks: the set
/// conversion, each 0 axis filled with the output's size (the scaled size
/// when it is set, else the video's); else, when the scaled size has both
/// axes and differs from the video's on either (`has_scaling()`), the
/// scaled size; else none.
#[must_use]
pub fn video_conversion(s: &ConversionSizes) -> Conversion {
    if s.set {
        let width = if s.set_width != 0 {
            s.set_width
        } else {
            raw_size(s.scaled_width, s.video_width)
        };
        let height = if s.set_height != 0 {
            s.set_height
        } else {
            raw_size(s.scaled_height, s.video_height)
        };
        return Conversion::UseSet { width, height };
    }
    let has_scaling = s.scaled_width != 0
        && s.scaled_height != 0
        && (s.video_width != s.scaled_width || s.video_height != s.scaled_height);
    if has_scaling {
        Conversion::Scaled {
            width: s.scaled_width,
            height: s.scaled_height,
        }
    } else {
        Conversion::None
    }
}

/// `obs_output_get_congestion()`'s clamp: below 0 is 0, above 1 is 1, the
/// rest unchanged, bit for bit (-0 stays -0). A NaN is neither, so it
/// passes through with its payload, as in C. `f32::clamp` makes the C's
/// two comparisons in the C's order (`< min`, then `> max`), so it gives
/// the same bits on every input; its bounds are constants, so it cannot
/// panic.
#[must_use]
pub fn congestion(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}
