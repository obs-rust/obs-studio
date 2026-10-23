//! Rust ports of the units extracted from the libobs core, one module per
//! file: so far `libobs/obs-video-plan.c.h` and `libobs/obs-context-names.c.h`
//! (from `obs.c.h`), `libobs/obs-module-paths.c.h` and
//! `libobs/obs-module-names.c.h` (from `obs-module.c.h`) and
//! `libobs/obs-audio-mix.c.h` (from `obs-audio.c.h`).
//!
//! The safe cores are [`video_plan`], [`context_names`], [`module_paths`],
//! [`module_names`] and [`audio_mix`]. The C ABI shims live in [`ffi`].

#![deny(unsafe_code)]

pub mod audio_mix;
pub mod context_names;
pub mod module_names;
pub mod module_paths;
mod platform;
pub mod video_plan;

#[allow(unsafe_code)]
pub mod ffi;
