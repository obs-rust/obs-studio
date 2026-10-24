//! Safe core of `libobs/obs-source-draw-plan.c.h`: the source draw
//! decisions of `obs-source.c`.
//!
//! - [`rotation_x`] and [`rotation_y`] (`rotate_async_video()`): the
//!   translate that moves a rotated async frame back into its box;
//! - [`async_linear_srgb`] and [`async_nonlinear_alpha`]
//!   (`obs_source_render_async_video()`): the linear sRGB flag and the
//!   non-linear alpha blend of an async frame;
//! - [`technique`]: the technique for each pair of source and current colour
//!   spaces, of an async frame and of `source_render()`'s conversion;
//! - [`reads_white_level`] and [`multiplier`]: when the SDR white level is
//!   read and the multiplier, for both;
//! - [`convert`] (`source_render()`): whether the source is drawn through
//!   `color_space_texrender`.
//!
//! `obs-source.c.h` keeps every `gs_*` call in its order and under the same
//! conditions, `convert_video_space()`, `obs_get_video_sdr_white_level()`,
//! `gs_get_format_from_space()` and the texrenders. It passes their results
//! in and acts on what comes back. Nothing here keeps state, takes a lock or
//! calls libobs.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

use std::os::raw::c_long;

pub use crate::draw_technique::{
    GS_CS_709_EXTENDED, GS_CS_709_SCRGB, GS_CS_SRGB, GS_CS_SRGB_16F, Technique,
};

/// `obs_source_draw_plan_rotation_x()`.
pub fn rotation_x(rotation: c_long, width: u32, height: u32) -> f32 {
    let _ = (rotation, width, height);
    todo!()
}

/// `obs_source_draw_plan_rotation_y()`.
pub fn rotation_y(rotation: c_long, width: u32, height: u32) -> f32 {
    let _ = (rotation, width, height);
    todo!()
}

/// `obs_source_draw_plan_async_linear_srgb()`.
pub fn async_linear_srgb(source_space: i32, current_space: i32, gs_linear_srgb: bool) -> bool {
    let _ = (source_space, current_space, gs_linear_srgb);
    todo!()
}

/// `obs_source_draw_plan_async_nonlinear_alpha()`.
pub fn async_nonlinear_alpha(source_space: i32, linear_srgb: bool, linear_alpha: bool) -> bool {
    let _ = (source_space, linear_srgb, linear_alpha);
    todo!()
}

/// `obs_source_draw_plan_technique()`.
pub fn technique(source_space: i32, current_space: i32, nonlinear_alpha: bool) -> Technique {
    let _ = (source_space, current_space, nonlinear_alpha);
    todo!()
}

/// `obs_source_draw_plan_reads_white_level()`.
pub fn reads_white_level(source_space: i32, current_space: i32) -> bool {
    let _ = (source_space, current_space);
    todo!()
}

/// `obs_source_draw_plan_multiplier()`.
pub fn multiplier(source_space: i32, current_space: i32, sdr_white_level: f32) -> f32 {
    let _ = (source_space, current_space, sdr_white_level);
    todo!()
}

/// `obs_source_draw_plan_convert()`.
pub fn convert(source_space: i32, current_space: i32) -> bool {
    let _ = (source_space, current_space);
    todo!()
}
