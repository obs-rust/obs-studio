//! Safe core of `libobs/obs-video-copy-plan.c.h`: the output frame copy plan
//! and the scale effect choice of `obs-video.c`.
//!
//! `obs-video.c` keeps every `memcpy()` (it runs the plan), the frame lock
//! and unlock, `video_output_get_info()`, the effect pointers (it maps
//! [`ScaleEffect`] to `obs->video`'s) and the walk of the mix list. It reads
//! the values, passes them here, and does what the answer says. These keep
//! no state, take no lock and touch no frame: they only compute numbers.
//!
//! - [`gpu_copy_plan`] (`set_gpu_converted_data()`): per format, the copy of
//!   each plane of a GPU-converted frame into the output frame: one copy of
//!   `row * rows` bytes when the row size equals both line sizes, else
//!   `rows` copies of the row size at the two strides. The UV plane of a
//!   packed NV12 or P010 frame (input `linesize[1]` of 0) follows plane 0 in
//!   input plane 0.
//! - [`rgbx_copy_plan`] (`copy_rgbx_frame()`): one copy of `linesize *
//!   height` when the line sizes match, else rows of `width * 4`.
//! - [`resolution_close`] (`resolution_close()`): both axes within 16 of
//!   the base size.
//! - [`scale_effect`] (`get_scale_effect()` with
//!   `get_scale_effect_internal()`): which effect scales the output.
//! - [`mix_reuse_match`] (one comparison of `can_reuse_mix_texture()`).
//!
//! The arithmetic is the C's, wrap for wrap: the row sizes are `u32`
//! (`width * 2` and `width * 4` wrap, "characterized, not endorsed"), the
//! single copy's size and the RGBX row size are `size_t` products
//! (`usize`, wrapping as `size_t` does). `resolution_close()` subtracts in
//! `long`; here it is the distance of the two `u32`s, which is what an
//! `i64` subtraction gives for every pair, and what a 32-bit `long` (MSVC)
//! gives for every size below 2^31, so for every size
//! `obs_video_size_valid()` allows and every texture size.
//!
//! No input comes from outside OBS (the format and sizes come from
//! `obs_reset_video()`, the line sizes from the graphics driver and
//! video-io), but nothing here can panic on any input.
//!
//! Intentional difference, MSVC only and unreachable: with a 32-bit `long`
//! the C's `resolution_close()` turns a size of 2^31 or more negative and its
//! subtraction can overflow (undefined); the port answers as an `i64`
//! subtraction does. No texture or video size gets near 2^31.

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

use obs_media_io::video_io::VideoFormat;

use crate::video_plan::ObsScaleType;

/// At most one copy per GPU conversion plane (`NUM_CHANNELS`).
pub const COPY_MAX: usize = 3;

/// The line sizes the plan reads: the first three of a frame's
/// `MAX_AV_PLANES`.
pub type LineSizes = [u32; COPY_MAX];

/// One copy into a plane of the output frame (`struct obs_video_copy`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FrameCopy {
    /// The source starts where copy 0's source ended (the UV plane of a
    /// packed NV12 or P010 frame), not at its own input plane.
    pub src_after_plane0: bool,
    /// One copy of [`bytes`](Self::bytes); the source then moves on by it.
    pub whole: bool,
    /// The size of the one copy when [`whole`](Self::whole), else of each
    /// row.
    pub bytes: usize,
    /// The rows when not [`whole`](Self::whole). Set either way.
    pub rows: u32,
    /// How far the source moves on after each row.
    pub src_stride: u32,
    /// How far the destination moves on after each row.
    pub dst_stride: u32,
}

/// The copies of one frame (`struct obs_video_copy_plan`): copy `i` writes
/// output plane `i` and reads input plane `i`, or plane 0's continuation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CopyPlan {
    /// How many of [`copies`](Self::copies) are used.
    pub count: usize,
    /// The copies, in order; the rest are zero.
    pub copies: [FrameCopy; COPY_MAX],
}

impl CopyPlan {
    /// The copies in use.
    #[must_use]
    pub fn used(&self) -> &[FrameCopy] {
        self.copies.get(..self.count).unwrap_or(&[])
    }

    fn push(&mut self, copy: FrameCopy) {
        if let Some(slot) = self.copies.get_mut(self.count) {
            *slot = copy;
            self.count = self.count.saturating_add(1);
        }
    }
}

/// The effect `render_output_texture()` scales with (`enum
/// obs_video_scale_effect`); `obs-video.c` maps it to `obs->video`'s effect
/// of the same name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScaleEffect {
    /// `default_effect`.
    Default,
    /// `bilinear_lowres_effect`.
    BilinearLowres,
    /// `bicubic_effect`.
    Bicubic,
    /// `lanczos_effect`.
    Lanczos,
    /// `area_effect`.
    Area,
}

impl ScaleEffect {
    /// Every effect, in the C enum's order.
    pub const ALL: [Self; 5] = [
        Self::Default,
        Self::BilinearLowres,
        Self::Bicubic,
        Self::Lanczos,
        Self::Area,
    ];

    /// `OBS_VIDEO_SCALE_LOADED()`: the bit of [`ScaleInput::loaded`] that
    /// says this effect loaded.
    #[must_use]
    pub const fn loaded_bit(self) -> u32 {
        match self {
            Self::Default => 0x1,
            Self::BilinearLowres => 0x2,
            Self::Bicubic => 0x4,
            Self::Lanczos => 0x8,
            Self::Area => 0x10,
        }
    }
}

/// What `get_scale_effect()` looks at (`struct obs_video_scale_input`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ScaleInput {
    /// `mix->ovi.base_width`.
    pub base_width: u32,
    /// `mix->ovi.base_height`.
    pub base_height: u32,
    /// The output texture's width.
    pub target_width: u32,
    /// The output texture's height.
    pub target_height: u32,
    /// The video output's width; not looked at when the target is close to
    /// the base size.
    pub output_width: u32,
    /// The video output's height; as `output_width`.
    pub output_height: u32,
    /// `mix->ovi.scale_type`; `None` outside `enum obs_scale_type` (the C's
    /// `default:`).
    pub scale_type: Option<ObsScaleType>,
    /// [`ScaleEffect::loaded_bit`]s of the effects that loaded.
    pub loaded: u32,
}

/// What `can_reuse_mix_texture()` compares of a mix (`struct
/// obs_video_mix_reuse_key`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MixReuseKey {
    /// `mix->view`, as a number.
    pub view: usize,
    /// `mix->render_space` (`enum gs_color_space`).
    pub render_space: i32,
    /// `mix->ovi.base_width`.
    pub base_width: u32,
    /// `mix->ovi.base_height`.
    pub base_height: u32,
    /// `mix->texture_rendered`.
    pub rendered: bool,
}

/// `size_t` of a `u32`.
#[allow(clippy::as_conversions)]
const fn size(value: u32) -> usize {
    value as usize
}

/// One plane (`set_gpu_converted_plane()`): one copy when the row size and
/// both line sizes match, else a row at a time.
fn plane(
    row: u32,
    rows: u32,
    linesize_input: u32,
    linesize_output: u32,
    src_after_plane0: bool,
) -> FrameCopy {
    let whole = row == linesize_input && row == linesize_output;
    FrameCopy {
        src_after_plane0,
        whole,
        bytes: if whole {
            size(row).wrapping_mul(size(rows))
        } else {
            size(row)
        },
        rows,
        src_stride: linesize_input,
        dst_stride: linesize_output,
    }
}

/// `set_gpu_converted_data()`: the copies of a frame of `format` (`None`
/// outside `enum video_format`) at `width` x `height`, from a GPU-converted
/// frame with the line sizes `input` into one with `output`. No copy for a
/// format without a GPU conversion.
#[must_use]
pub fn gpu_copy_plan(
    format: Option<VideoFormat>,
    width: u32,
    height: u32,
    input: &LineSizes,
    output: &LineSizes,
) -> CopyPlan {
    let [in0, in1, in2] = *input;
    let [out0, out1, out2] = *output;
    let width_d2 = width / 2;
    let height_d2 = height / 2;
    let width_x2 = width.wrapping_mul(2);
    let width_x4 = width.wrapping_mul(4);
    let mut plan = CopyPlan::default();

    match format {
        Some(VideoFormat::I420) => {
            plan.push(plane(width, height, in0, out0, false));
            plan.push(plane(width_d2, height_d2, in1, out1, false));
            plan.push(plane(width_d2, height_d2, in2, out2, false));
        }
        Some(VideoFormat::Nv12) => {
            plan.push(plane(width, height, in0, out0, false));
            if in1 != 0 {
                plan.push(plane(width, height_d2, in1, out1, false));
            } else {
                plan.push(plane(width, height_d2, in0, out1, true));
            }
        }
        Some(VideoFormat::I444) => {
            plan.push(plane(width, height, in0, out0, false));
            plan.push(plane(width, height, in1, out1, false));
            plan.push(plane(width, height, in2, out2, false));
        }
        Some(VideoFormat::I010) => {
            plan.push(plane(width_x2, height, in0, out0, false));
            plan.push(plane(width, height_d2, in1, out1, false));
            plan.push(plane(width, height_d2, in2, out2, false));
        }
        Some(VideoFormat::P010) => {
            plan.push(plane(width_x2, height, in0, out0, false));
            if in1 != 0 {
                plan.push(plane(width_x2, height_d2, in1, out1, false));
            } else {
                plan.push(plane(width_x2, height_d2, in0, out1, true));
            }
        }
        Some(VideoFormat::P216) => {
            plan.push(plane(width_x2, height, in0, out0, false));
            plan.push(plane(width_x2, height, in1, out1, false));
        }
        Some(VideoFormat::P416) => {
            plan.push(plane(width_x2, height, in0, out0, false));
            plan.push(plane(width_x4, height, in1, out1, false));
        }
        Some(
            VideoFormat::None
            | VideoFormat::Yvyu
            | VideoFormat::Yuy2
            | VideoFormat::Uyvy
            | VideoFormat::Rgba
            | VideoFormat::Bgra
            | VideoFormat::Bgrx
            | VideoFormat::Y800
            | VideoFormat::Bgr3
            | VideoFormat::I412
            | VideoFormat::I422
            | VideoFormat::I210
            | VideoFormat::I40a
            | VideoFormat::I42a
            | VideoFormat::Yuva
            | VideoFormat::Ya2l
            | VideoFormat::Ayuv
            | VideoFormat::V210
            | VideoFormat::R10l,
        )
        | None => {}
    }

    plan
}

/// `copy_rgbx_frame()`: one copy of `linesize * height` when the line sizes
/// match (even when they are under `width * 4`), else `height` rows of
/// `width * 4` bytes (a `size_t` product, which does not wrap at 2^32 on a
/// 64-bit host).
#[must_use]
pub fn rgbx_copy_plan(width: u32, height: u32, in_linesize: u32, out_linesize: u32) -> FrameCopy {
    let whole = in_linesize == out_linesize;
    FrameCopy {
        src_after_plane0: false,
        whole,
        bytes: if whole {
            size(in_linesize).wrapping_mul(size(height))
        } else {
            size(width).wrapping_mul(4)
        },
        rows: height,
        src_stride: in_linesize,
        dst_stride: out_linesize,
    }
}

/// `resolution_close()`: `width` x `height` within 16 of the base size on
/// both axes. The distance is `|(long)base - (long)size|` in C; `abs_diff`
/// is that for every `u32` pair with a 64-bit `long`, and for every size
/// below 2^31 with a 32-bit one.
#[must_use]
pub fn resolution_close(base_width: u32, base_height: u32, width: u32, height: u32) -> bool {
    base_width.abs_diff(width) <= 16 && base_height.abs_diff(height) <= 16
}

/// `get_scale_effect_internal()`: the bilinear low resolution effect when
/// the output is under half the base size on both axes, else the scale
/// type's.
fn scale_effect_internal(input: &ScaleInput) -> ScaleEffect {
    if input.output_width < input.base_width / 2 && input.output_height < input.base_height / 2 {
        return ScaleEffect::BilinearLowres;
    }
    match input.scale_type {
        Some(ObsScaleType::Bilinear) => ScaleEffect::Default,
        Some(ObsScaleType::Lanczos) => ScaleEffect::Lanczos,
        Some(ObsScaleType::Area) => ScaleEffect::Area,
        Some(ObsScaleType::Bicubic | ObsScaleType::Disable | ObsScaleType::Point) | None => {
            ScaleEffect::Bicubic
        }
    }
}

/// `get_scale_effect()`: the default effect when the target is close to the
/// base size ([`resolution_close`]); else
/// `get_scale_effect_internal()`'s, and when that one did not load, the
/// bicubic effect if it loaded, else the default (loaded or not).
#[must_use]
pub fn scale_effect(input: &ScaleInput) -> ScaleEffect {
    if resolution_close(
        input.base_width,
        input.base_height,
        input.target_width,
        input.target_height,
    ) {
        return ScaleEffect::Default;
    }
    let effect = scale_effect_internal(input);
    if input.loaded & effect.loaded_bit() != 0 {
        effect
    } else if input.loaded & ScaleEffect::Bicubic.loaded_bit() != 0 {
        ScaleEffect::Bicubic
    } else {
        ScaleEffect::Default
    }
}

/// One comparison of `can_reuse_mix_texture()`: `other` (an earlier mix)
/// can stand in for `mix` when they share the view, the render space and
/// the base size and `other` has rendered its texture. `mix.rendered` is
/// not looked at.
#[must_use]
pub fn mix_reuse_match(other: &MixReuseKey, mix: &MixReuseKey) -> bool {
    other.view == mix.view
        && other.render_space == mix.render_space
        && other.base_width == mix.base_width
        && other.base_height == mix.base_height
        && other.rendered
}
