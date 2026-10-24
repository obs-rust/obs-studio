//! C ABI for `libobs/obs-video-copy-plan.c.h`, the output frame copy plan
//! and the scale effect choice of `obs-video.c`, declared in
//! `libobs/obs-video-copy-plan.h` without `EXPORT`.
//!
//! These only compute numbers. `obs-video.c.h` (still C) runs every
//! `memcpy()` of the plan (`set_gpu_converted_plane()`, copy `i` from input
//! plane `i` or from plane 0's end into output plane `i`), locks and unlocks
//! the frame, calls `video_output_get_info()`, maps [`obs_video_scale_effect`]
//! to `obs->video`'s effect pointers and walks the mix list. The plan, the
//! scale input and the reuse keys cross as `#[repr(C)]` structs whose layout
//! Tier 3 checks against the C. No frame, effect or mix pointer comes here:
//! a mix's view crosses as a number and is only compared. All stay hidden
//! (`rust-hidden.txt`).
//!
//! The enums cross as `c_int`s; a format or scale type outside its enum
//! takes the C's `default:`. The one intentional difference is the core's
//! (`resolution_close()` with a 32-bit `long`, MSVC only, unreachable). A
//! null pointer is not checked: libobs passes a frame's `linesize` arrays
//! and locals.

use core::ffi::c_int;

use obs_media_io::video_io::video_format_from_c;

use crate::video_copy_plan::{
    self as copy_plan, COPY_MAX, CopyPlan, FrameCopy, LineSizes, MixReuseKey, ScaleEffect,
    ScaleInput,
};
use crate::video_plan::ObsScaleType;

/// `struct obs_video_copy` (`obs-video-copy-plan.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_video_copy {
    pub src_after_plane0: bool,
    pub whole: bool,
    pub bytes: usize,
    pub rows: u32,
    pub src_stride: u32,
    pub dst_stride: u32,
}

/// `struct obs_video_copy_plan` (`obs-video-copy-plan.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_video_copy_plan {
    pub count: usize,
    pub copies: [obs_video_copy; COPY_MAX],
}

/// `struct obs_video_scale_input` (`obs-video-copy-plan.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_video_scale_input {
    pub base_width: u32,
    pub base_height: u32,
    pub target_width: u32,
    pub target_height: u32,
    pub output_width: u32,
    pub output_height: u32,
    /// An `enum obs_scale_type` value.
    pub scale_type: c_int,
    pub loaded: u32,
}

/// `struct obs_video_mix_reuse_key` (`obs-video-copy-plan.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_video_mix_reuse_key {
    pub view: usize,
    /// An `enum gs_color_space` value.
    pub render_space: c_int,
    pub base_width: u32,
    pub base_height: u32,
    pub rendered: bool,
}

/// `enum obs_video_scale_effect`, as the C returns it (an `unsigned int`;
/// every value fits an `int` too).
#[allow(non_camel_case_types)]
pub type obs_video_scale_effect = u32;

/// `OBS_VIDEO_SCALE_EFFECT_DEFAULT`.
pub const OBS_VIDEO_SCALE_EFFECT_DEFAULT: obs_video_scale_effect = 0;
/// `OBS_VIDEO_SCALE_EFFECT_BILINEAR_LOWRES`.
pub const OBS_VIDEO_SCALE_EFFECT_BILINEAR_LOWRES: obs_video_scale_effect = 1;
/// `OBS_VIDEO_SCALE_EFFECT_BICUBIC`.
pub const OBS_VIDEO_SCALE_EFFECT_BICUBIC: obs_video_scale_effect = 2;
/// `OBS_VIDEO_SCALE_EFFECT_LANCZOS`.
pub const OBS_VIDEO_SCALE_EFFECT_LANCZOS: obs_video_scale_effect = 3;
/// `OBS_VIDEO_SCALE_EFFECT_AREA`.
pub const OBS_VIDEO_SCALE_EFFECT_AREA: obs_video_scale_effect = 4;

/// The C value of a [`ScaleEffect`].
#[must_use]
pub const fn scale_effect_to_c(effect: ScaleEffect) -> obs_video_scale_effect {
    match effect {
        ScaleEffect::Default => OBS_VIDEO_SCALE_EFFECT_DEFAULT,
        ScaleEffect::BilinearLowres => OBS_VIDEO_SCALE_EFFECT_BILINEAR_LOWRES,
        ScaleEffect::Bicubic => OBS_VIDEO_SCALE_EFFECT_BICUBIC,
        ScaleEffect::Lanczos => OBS_VIDEO_SCALE_EFFECT_LANCZOS,
        ScaleEffect::Area => OBS_VIDEO_SCALE_EFFECT_AREA,
    }
}

impl From<FrameCopy> for obs_video_copy {
    fn from(c: FrameCopy) -> Self {
        Self {
            src_after_plane0: c.src_after_plane0,
            whole: c.whole,
            bytes: c.bytes,
            rows: c.rows,
            src_stride: c.src_stride,
            dst_stride: c.dst_stride,
        }
    }
}

impl From<CopyPlan> for obs_video_copy_plan {
    fn from(plan: CopyPlan) -> Self {
        Self {
            count: plan.count,
            copies: plan.copies.map(obs_video_copy::from),
        }
    }
}

impl From<obs_video_mix_reuse_key> for MixReuseKey {
    fn from(k: obs_video_mix_reuse_key) -> Self {
        Self {
            view: k.view,
            render_space: k.render_space,
            base_width: k.base_width,
            base_height: k.base_height,
            rendered: k.rendered,
        }
    }
}

/// The first [`COPY_MAX`] line sizes, all the C reads.
///
/// # Safety
///
/// `lines` points to at least [`COPY_MAX`] readable `u32`s.
unsafe fn read_lines(lines: *const u32) -> LineSizes {
    // SAFETY: COPY_MAX readable entries (caller contract); copied.
    unsafe { lines.cast::<LineSizes>().read_unaligned() }
}

/// `struct obs_video_copy_plan obs_video_gpu_copy_plan(enum video_format
/// format, uint32_t width, uint32_t height, const uint32_t *in_linesize,
/// const uint32_t *out_linesize)`.
///
/// # Safety
///
/// As the C: `in_linesize` and `out_linesize` each point to at least three
/// readable `uint32_t`s (a frame's `MAX_AV_PLANES` line sizes).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_video_gpu_copy_plan(
    format: c_int,
    width: u32,
    height: u32,
    in_linesize: *const u32,
    out_linesize: *const u32,
) -> obs_video_copy_plan {
    // SAFETY: three readable line sizes each (caller contract).
    let (input, output) = unsafe { (read_lines(in_linesize), read_lines(out_linesize)) };
    copy_plan::gpu_copy_plan(video_format_from_c(format), width, height, &input, &output).into()
}

/// `struct obs_video_copy obs_video_rgbx_copy_plan(uint32_t width, uint32_t
/// height, uint32_t in_linesize, uint32_t out_linesize)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_video_rgbx_copy_plan(
    width: u32,
    height: u32,
    in_linesize: u32,
    out_linesize: u32,
) -> obs_video_copy {
    copy_plan::rgbx_copy_plan(width, height, in_linesize, out_linesize).into()
}

/// `bool obs_video_scale_resolution_close(uint32_t base_width, uint32_t
/// base_height, uint32_t width, uint32_t height)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_video_scale_resolution_close(
    base_width: u32,
    base_height: u32,
    width: u32,
    height: u32,
) -> bool {
    copy_plan::resolution_close(base_width, base_height, width, height)
}

/// `enum obs_video_scale_effect obs_video_scale_effect_choice(const struct
/// obs_video_scale_input *input)`.
///
/// # Safety
///
/// As the C: `input` points to a readable `struct obs_video_scale_input`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_video_scale_effect_choice(
    input: *const obs_video_scale_input,
) -> obs_video_scale_effect {
    // SAFETY: a readable input (caller contract). Copied, not borrowed.
    let i = unsafe { input.read() };
    scale_effect_to_c(copy_plan::scale_effect(&ScaleInput {
        base_width: i.base_width,
        base_height: i.base_height,
        target_width: i.target_width,
        target_height: i.target_height,
        output_width: i.output_width,
        output_height: i.output_height,
        scale_type: ObsScaleType::from_c(i.scale_type),
        loaded: i.loaded,
    }))
}

/// `bool obs_video_mix_reuse_match(const struct obs_video_mix_reuse_key
/// *other, const struct obs_video_mix_reuse_key *mix)`.
///
/// # Safety
///
/// As the C: `other` and `mix` each point to a readable `struct
/// obs_video_mix_reuse_key`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_video_mix_reuse_match(
    other: *const obs_video_mix_reuse_key,
    mix: *const obs_video_mix_reuse_key,
) -> bool {
    // SAFETY: two readable keys (caller contract). Copied, not borrowed.
    let (other, mix) = unsafe { (other.read(), mix.read()) };
    copy_plan::mix_reuse_match(&other.into(), &mix.into())
}
