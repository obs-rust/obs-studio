//! Rust ports of the units extracted from `libobs/obs-source.c`, one module
//! per file: so far `libobs/obs-source-audio-timing.c.h`, the audio
//! timestamp placement of `source_output_audio_data()`.
//!
//! The safe core is [`audio_timing`]. The C ABI shims live in [`ffi`].

#![deny(unsafe_code)]

pub mod audio_timing;

#[allow(unsafe_code)]
pub mod ffi;
