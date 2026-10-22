//! C ABI for `libobs/obs-video-plan.c.h`.
//!
//! `obs.c` calls the five functions inside libobs. They are declared in
//! `obs-internal.h` without `EXPORT`, so the shims stay hidden
//! (`libobs/cmake/rust-hidden.txt`). The enums cross as `c_int`s; a value
//! outside its enum takes the C's `default:` branch. The technique names
//! and the scale type names are `'static` NUL-terminated strings, which
//! `obs-video.c` keeps and looks up by name.
//!
//! Intentional difference: a null pointer to `obs_video_align_output_size`
//! is skipped (C dereferences it); the other pointer is still aligned.

use core::ffi::{c_char, c_int};
use core::ptr;

use obs_media_io::video_io::{video_colorspace_from_c, video_format_from_c};

use crate::video_plan::{
    NUM_CHANNELS, ObsScaleType, OutputPlan, VideoPlane, align_output_size, output_plan,
    output_size_valid, scale_type_name, size_valid,
};

/// `struct obs_video_plane` (`obs-internal.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct obs_video_plane {
    pub width: u32,
    pub height: u32,
    /// `enum gs_color_format`; `GS_UNKNOWN` is no plane.
    pub format: c_int,
}

/// `struct obs_video_output_plan` (`obs-internal.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
pub struct obs_video_output_plan {
    pub conversion_needed: bool,
    pub conversion_techs: [*const c_char; NUM_CHANNELS],
    pub conversion_width_i: f32,
    pub conversion_height_i: f32,
    pub planes: [obs_video_plane; NUM_CHANNELS],
    /// `enum gs_color_format`.
    pub texture_format: c_int,
    /// `enum gs_color_space`.
    pub render_space: c_int,
}

impl From<VideoPlane> for obs_video_plane {
    fn from(plane: VideoPlane) -> Self {
        Self {
            width: plane.width,
            height: plane.height,
            format: plane.format.to_c(),
        }
    }
}

impl From<OutputPlan> for obs_video_output_plan {
    fn from(plan: OutputPlan) -> Self {
        Self {
            conversion_needed: plan.conversion_needed,
            conversion_techs: plan
                .conversion_techs
                .map(|t| t.map_or(ptr::null(), |t| t.as_ptr())),
            conversion_width_i: plan.conversion_width_i,
            conversion_height_i: plan.conversion_height_i,
            planes: plan.planes.map(obs_video_plane::from),
            texture_format: plan.texture_format.to_c(),
            render_space: plan.render_space.to_c(),
        }
    }
}

/// Same contract as C `obs_video_size_valid`. No pointers.
#[unsafe(no_mangle)]
pub extern "C" fn obs_video_size_valid(width: u32, height: u32) -> bool {
    size_valid(width, height)
}

/// Same contract as C `obs_video_align_output_size`, except that a null
/// pointer is skipped.
///
/// # Safety
///
/// `width` and `height` are each null or valid for reads and writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_video_align_output_size(width: *mut u32, height: *mut u32) {
    // SAFETY: the caller passes null or valid pointers; null is skipped.
    let (w, h) = unsafe { (width.as_mut(), height.as_mut()) };
    let (aw, ah) = align_output_size(
        w.as_deref().copied().unwrap_or(0),
        h.as_deref().copied().unwrap_or(0),
    );
    if let Some(w) = w {
        *w = aw;
    }
    if let Some(h) = h {
        *h = ah;
    }
}

/// Same contract as C `obs_video_output_size_valid`. No pointers.
#[unsafe(no_mangle)]
pub extern "C" fn obs_video_output_size_valid(width: u32, height: u32) -> bool {
    output_size_valid(width, height)
}

/// Same contract as C `obs_video_scale_type_name`: a static string, `""`
/// outside `enum obs_scale_type`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_video_scale_type_name(scale_type: c_int) -> *const c_char {
    scale_type_name(ObsScaleType::from_c(scale_type)).as_ptr()
}

/// Same contract as C `obs_video_output_plan`, returned by value. No
/// pointers in; the technique pointers are static.
#[unsafe(no_mangle)]
pub extern "C" fn obs_video_output_plan(
    format: c_int,
    colorspace: c_int,
    width: u32,
    height: u32,
) -> obs_video_output_plan {
    output_plan(
        video_format_from_c(format),
        video_colorspace_from_c(colorspace),
        width,
        height,
    )
    .into()
}
