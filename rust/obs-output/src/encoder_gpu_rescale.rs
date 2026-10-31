//! Safe core of `libobs/obs-encoder-gpu-rescale.c.h`: the GPU rescale mix
//! decisions of `obs-encoder.c`.
//!
//! An encoder with a GPU scale type and a scaled size, or a preferred format,
//! colorspace or range, encodes from an encoder-only video mix of the same
//! view:
//! - [`wanted`]: the gates of `maybe_set_up_gpu_rescale()`: no media,
//!   `OBS_SCALE_DISABLE`, or nothing requested;
//! - [`target`]: the size, format, colorspace and range the mix must
//!   output: the encoder's value, else the video output's;
//! - [`matches`]: whether a mix serves that target (same view and scale
//!   type, an output of the target size, format, colorspace and range);
//!   `maybe_set_up_gpu_rescale()` asks it in its first search and again
//!   after it created a mix;
//! - [`release_step`] and [`release_refs`]:
//!   `maybe_clear_encoder_core_video_mix()`, which mix an encoder's shutdown
//!   releases, and when the mix's last reference goes.
//!
//! `obs-encoder.c` keeps `get_mix_for_video()`, `video_output_get_info()`,
//! the mixes array and `mixes_mutex`, the reads and writes of
//! `encoder_refs`, `original_video` and `encoder_only_mix`, the mix
//! creation and free, and `obs_encoder_set_video()` / `encoder_set_video()`.
//! These keep no state and call nothing in libobs.
//!
//! The enums come in as `int`, unchecked, as the C passes them: any value
//! other than the "none" or "default" one counts as set. Nothing here can
//! panic on any input.
//!
//! No intentional differences.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

/// `OBS_SCALE_DISABLE` (`enum obs_scale_type`, `obs.h`).
pub const OBS_SCALE_DISABLE: i32 = 0;
/// `VIDEO_FORMAT_NONE` (`enum video_format`, `media-io/video-io.h`).
pub const VIDEO_FORMAT_NONE: i32 = 0;
/// `VIDEO_CS_DEFAULT` (`enum video_colorspace`).
pub const VIDEO_CS_DEFAULT: i32 = 0;
/// `VIDEO_RANGE_DEFAULT` (`enum video_range_type`).
pub const VIDEO_RANGE_DEFAULT: i32 = 0;

/// What an encoder asks of its rescale: `struct obs_encoder`'s
/// `gpu_scale_type`, `scaled_width`, `scaled_height`, `preferred_format`,
/// `preferred_space` and `preferred_range`, the enums as `int`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Request {
    /// `enum obs_scale_type`.
    pub scale_type: i32,
    /// The scaled width, 0 for none.
    pub scaled_width: u32,
    /// The scaled height, 0 for none.
    pub scaled_height: u32,
    /// `enum video_format`.
    pub format: i32,
    /// `enum video_colorspace`.
    pub colorspace: i32,
    /// `enum video_range_type`.
    pub range: i32,
}

/// A video output's size, format, colorspace and range: the fields of
/// `struct video_output_info` a mix is matched on, the enums as `int`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Output {
    /// The width.
    pub width: u32,
    /// The height.
    pub height: u32,
    /// `enum video_format`.
    pub format: i32,
    /// `enum video_colorspace`.
    pub colorspace: i32,
    /// `enum video_range_type`.
    pub range: i32,
}

/// Which mix `maybe_clear_encoder_core_video_mix()` releases, for each mix
/// in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Release {
    /// Not the encoder's mix: the next one.
    Skip,
    /// The encoder's mix, but not an encoder-only one: nothing to release.
    Stop,
    /// The encoder's encoder-only mix: back to the original video, and the
    /// mix loses one reference.
    Drop,
}

/// `obs_encoder_gpu_rescale_wanted()`: false when the encoder has no media,
/// its scale type is `OBS_SCALE_DISABLE`, or it asks for no scaled size and
/// no preferred format, colorspace or range.
#[must_use]
pub fn wanted(has_media: bool, request: &Request) -> bool {
    if !has_media {
        return false;
    }
    if request.scale_type == OBS_SCALE_DISABLE {
        return false;
    }
    let nothing = request.scaled_height == 0
        && request.scaled_width == 0
        && request.format == VIDEO_FORMAT_NONE
        && request.colorspace == VIDEO_CS_DEFAULT
        && request.range == VIDEO_RANGE_DEFAULT;
    !nothing
}

/// `obs_encoder_gpu_rescale_target()`: each field the request's when it is
/// set (a scaled width or height other than 0; a format, colorspace or range
/// other than the "none" or "default" value), else `video`'s.
#[must_use]
pub fn target(request: &Request, video: &Output) -> Output {
    let pick = |set: bool, own: i32, video: i32| if set { own } else { video };
    Output {
        width: if request.scaled_width != 0 {
            request.scaled_width
        } else {
            video.width
        },
        height: if request.scaled_height != 0 {
            request.scaled_height
        } else {
            video.height
        },
        format: pick(
            request.format != VIDEO_FORMAT_NONE,
            request.format,
            video.format,
        ),
        colorspace: pick(
            request.colorspace != VIDEO_CS_DEFAULT,
            request.colorspace,
            video.colorspace,
        ),
        range: pick(
            request.range != VIDEO_RANGE_DEFAULT,
            request.range,
            video.range,
        ),
    }
}

/// `obs_encoder_gpu_rescale_matches()`: a mix serves the target when it
/// belongs to the same view, has the encoder's scale type, and its video
/// output has the target's size, format, colorspace and range.
#[must_use]
pub fn matches(
    same_view: bool,
    mix_scale_type: i32,
    mix: &Output,
    scale_type: i32,
    target: &Output,
) -> bool {
    same_view && mix_scale_type == scale_type && mix == target
}

/// `obs_encoder_gpu_rescale_release_step()`: `is_media` when the mix's video
/// is the encoder's media, `encoder_only` the mix's `encoder_only_mix`.
#[must_use]
pub fn release_step(is_media: bool, encoder_only: bool) -> Release {
    if !is_media {
        Release::Skip
    } else if !encoder_only {
        Release::Stop
    } else {
        Release::Drop
    }
}

/// `obs_encoder_gpu_rescale_release_refs()`: the mix's `encoder_refs` after
/// one reference goes, and whether that was the last one. The C's `long` is
/// `i64` here (LP64; the shim converts on LLP64 Windows); a count of
/// `i64::MIN` wraps instead of the C's undefined overflow.
#[must_use]
pub fn release_refs(encoder_refs: i64) -> (i64, bool) {
    let after = encoder_refs.wrapping_sub(1);
    (after, after == 0)
}
