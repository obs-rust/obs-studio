//! Rust ports of `libobs/media-io`.
//!
//! Among them, [`audio_resampler_drift`] is the libswresample resampler's
//! clock drift loop and output frame padding
//! (`libobs/media-io/audio-resampler-drift.c.h`).
//!
//! Safe cores live at the crate root; the C ABI shims that replace the
//! original `EXPORT` symbols live in [`ffi`]. See
//! `docs/rust-port/testing-policy.md`. Host-specific code lives only in
//! `platform` (and the per-OS export set in [`ffi`]).

#![deny(unsafe_code)]

#[cfg(test)]
use obs_c_oracle as _; // links the test bmalloc/bfree (oracle/test_bmem.c)

pub mod audio_io;
pub mod audio_math;
pub mod audio_resampler_drift;
#[allow(unsafe_code)] // the C ABI shims
pub mod ffi;
pub mod format_conversion;
pub mod frame_rate;
pub mod media_remux;
mod platform;
pub mod video_fourcc;
pub mod video_frame;
pub mod video_io;
pub mod video_matrices;
mod video_output;

#[cfg(test)]
extern crate self as obs_media_io;

#[cfg(test)]
#[allow(unsafe_code)] // Existing layout and C-oracle tests retain their FFI boundary.
#[path = "../tests/suite.rs"]
mod tests;
