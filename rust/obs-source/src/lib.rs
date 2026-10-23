//! Rust ports of the units extracted from `libobs/obs-source.c` and
//! `libobs/obs-source-deinterlace.c`, one module per file: so far
//! `libobs/obs-source-audio-timing.c.h`, the audio timestamp placement of
//! `source_output_audio_data()`, and `libobs/obs-source-deinterlace-pacing.c.h`,
//! the frame pacing of double-rate deinterlacing.
//!
//! The safe cores are [`audio_timing`] and [`deinterlace_pacing`]. The C ABI
//! shims live in [`ffi`].

#![deny(unsafe_code)]

pub mod audio_timing;
pub mod deinterlace_pacing;

#[allow(unsafe_code)]
pub mod ffi;
