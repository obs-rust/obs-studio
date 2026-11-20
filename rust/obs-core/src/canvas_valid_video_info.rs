//! Safe core of `libobs/obs-canvas-has-valid-video-info.c.h`:
//! `obs_canvas_has_valid_video_info()`, whether a canvas's stored video info
//! can reset its mix.
//!
//! Five conjuncts, in the C's order and with its short circuit: the base
//! width and height and the output width and height are set
//! ([`canvas_rules::size_set`]) and the output format is set
//! ([`canvas_rules::format_set`], so a format outside the enum counts as
//! set). Each field is read only once its earlier conjuncts held; nothing is
//! written, locked or allocated.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

use core::ffi::c_int;

use crate::canvas_rules::{format_set, size_set};

/// The canvas's `ovi` fields the check reads, one read per call.
pub trait VideoInfoFields {
    fn base_width(&mut self) -> u32;
    fn base_height(&mut self) -> u32;
    fn output_width(&mut self) -> u32;
    fn output_height(&mut self) -> u32;
    /// `(int)ovi->output_format`.
    fn output_format(&mut self) -> c_int;
}

/// True when all four sizes and the output format are set.
pub fn has_valid_video_info(fields: &mut impl VideoInfoFields) -> bool {
    size_set(fields.base_width())
        && size_set(fields.base_height())
        && size_set(fields.output_width())
        && size_set(fields.output_height())
        && format_set(fields.output_format())
}
