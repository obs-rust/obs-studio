//! Safe core of `libobs/obs-source-frame-wrappers.c.h`: the six exported
//! async-frame entry points `obs_source_output_video()`,
//! `obs_source_preload_video()`, `obs_source_set_video_frame()` and their
//! `*2()` variants.
//!
//! Each is the same thin wrapper in front of a worker that stays C
//! (`obs_source_output_video_internal()`, `_preload_video_internal()`,
//! `_set_video_frame_internal()`):
//! - a source being destroyed returns at once, before the NULL check;
//! - a missing frame goes to the worker as NULL; the two `set_video_frame`
//!   entries hand it to the *preload* worker, not the set worker
//!   (characterized, not endorsed);
//! - an `obs_source_frame` is copied whole with only `full_range` replaced
//!   by [`frame_copy::full_range`];
//! - an `obs_source_frame2` gives a zeroed frame of its planes, size,
//!   timestamp, format, flip, flags, transfer and colour matrix and range,
//!   with `full_range` from [`frame_copy::full_range2`] and
//!   `max_luminance` 0.
//!
//! The worker gets the local copy, never the caller's frame, which is never
//! written. Floats and pointers are copied as they are.
//!
//! No intentional differences from C.

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

use core::ffi::{c_int, c_long};

use crate::frame_copy;

/// `MAX_AV_PLANES` (`media-io/media-io.h`).
pub const MAX_AV_PLANES: usize = 8;

/// `struct obs_source_frame` (`obs.h`), field for field.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    pub data: [*mut u8; MAX_AV_PLANES],
    pub linesize: [u32; MAX_AV_PLANES],
    pub width: u32,
    pub height: u32,
    pub timestamp: u64,
    /// `enum video_format`, an `int`.
    pub format: c_int,
    pub color_matrix: [f32; 16],
    pub full_range: bool,
    pub max_luminance: u16,
    pub color_range_min: [f32; 3],
    pub color_range_max: [f32; 3],
    pub flip: bool,
    pub flags: u8,
    /// `enum video_trc`.
    pub trc: u8,
    /// `volatile long refs`.
    pub refs: c_long,
    pub prev_frame: bool,
}

/// `struct obs_source_frame2` (`obs.h`), field for field.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Frame2 {
    pub data: [*mut u8; MAX_AV_PLANES],
    pub linesize: [u32; MAX_AV_PLANES],
    pub width: u32,
    pub height: u32,
    pub timestamp: u64,
    /// `enum video_format`, an `int`.
    pub format: c_int,
    /// `enum video_range_type`, an `int`.
    pub range: c_int,
    pub color_matrix: [f32; 16],
    pub color_range_min: [f32; 3],
    pub color_range_max: [f32; 3],
    pub flip: bool,
    pub flags: u8,
    /// `enum video_trc`.
    pub trc: u8,
}

/// The source and the three C workers the entry points call.
pub trait Host {
    /// A source as the caller passed it.
    type Source: Copy;
    /// `destroying(source)`.
    fn destroying(&self, source: Self::Source) -> bool;
    /// `obs_source_output_video_internal(source, frame)`.
    fn output(&self, source: Self::Source, frame: Option<&Frame>);
    /// `obs_source_preload_video_internal(source, frame)`.
    fn preload(&self, source: Self::Source, frame: Option<&Frame>);
    /// `obs_source_set_video_frame_internal(source, frame)`.
    fn set(&self, source: Self::Source, frame: Option<&Frame>);
}

/// `struct obs_source_frame new_frame = *frame;` with `full_range`
/// replaced.
#[must_use]
pub fn from_frame(frame: &Frame) -> Frame {
    todo!("obs_source_frame copy (#5760): {frame:?}")
}

/// The `obs_source_frame` an `obs_source_frame2` becomes.
#[must_use]
pub fn from_frame2(frame: &Frame2) -> Frame {
    todo!("obs_source_frame2 build (#5760): {frame:?}")
}

/// `obs_source_output_video(source, frame)`.
pub fn output_video<H: Host>(host: &H, source: H::Source, frame: Option<&Frame>) {
    let _ = (host, source, frame);
    todo!("obs_source_output_video safe core (#5760)")
}

/// `obs_source_output_video2(source, frame)`.
pub fn output_video2<H: Host>(host: &H, source: H::Source, frame: Option<&Frame2>) {
    let _ = (host, source, frame);
    todo!("obs_source_output_video2 safe core (#5760)")
}

/// `obs_source_preload_video(source, frame)`.
pub fn preload_video<H: Host>(host: &H, source: H::Source, frame: Option<&Frame>) {
    let _ = (host, source, frame);
    todo!("obs_source_preload_video safe core (#5760)")
}

/// `obs_source_preload_video2(source, frame)`.
pub fn preload_video2<H: Host>(host: &H, source: H::Source, frame: Option<&Frame2>) {
    let _ = (host, source, frame);
    todo!("obs_source_preload_video2 safe core (#5760)")
}

/// `obs_source_set_video_frame(source, frame)`.
pub fn set_video_frame<H: Host>(host: &H, source: H::Source, frame: Option<&Frame>) {
    let _ = (host, source, frame, frame_copy::full_range);
    todo!("obs_source_set_video_frame safe core (#5760)")
}

/// `obs_source_set_video_frame2(source, frame)`.
pub fn set_video_frame2<H: Host>(host: &H, source: H::Source, frame: Option<&Frame2>) {
    let _ = (host, source, frame);
    todo!("obs_source_set_video_frame2 safe core (#5760)")
}
