//! Rust ports of the units extracted from `libobs/obs-source.c` and
//! `libobs/obs-source-deinterlace.c`, one module per file: so far
//! `libobs/obs-source-audio-timing.c.h`, the audio timestamp placement of
//! `source_output_audio_data()`; `libobs/obs-source-color-space.c.h`, the
//! colour space query routing of `obs_source_get_color_space()`; and
//! `libobs/obs-source-deinterlace-pacing.c.h`, the frame pacing of
//! double-rate deinterlacing.
//!
//! The safe cores are [`audio_timing`], [`color_space`] and
//! [`deinterlace_pacing`]. The C ABI shims live in [`ffi`].

#![deny(unsafe_code)]

pub mod audio_timing;
pub mod color_space;
pub mod deinterlace_pacing;

#[allow(unsafe_code)]
pub mod ffi;
