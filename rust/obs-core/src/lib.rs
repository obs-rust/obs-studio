//! Rust ports of the units extracted from the libobs core, one module per
//! file: so far `libobs/obs-video-plan.c.h` and `libobs/obs-context-names.c.h`
//! (from `obs.c.h`), `libobs/obs-module-paths.c.h`,
//! `libobs/obs-module-names.c.h` and `libobs/obs-module-locale.c.h` (from
//! `obs-module.c.h`) and `libobs/obs-audio-mix.c.h` (from `obs-audio.c.h`); and
//! `libobs/util/source-profiler.c.h`, which reads core state (sources and the
//! graphics context) and so belongs to the core rather than obs-util.
//!
//! The safe cores are [`video_plan`], [`context_names`], [`module_paths`],
//! [`module_names`], [`module_locale`], [`audio_mix`] and [`source_profiler`]. The C ABI shims live in [`ffi`].

#![deny(unsafe_code)]

pub mod audio_mix;
pub mod context_names;
pub mod module_locale;
pub mod module_names;
pub mod module_paths;
mod platform;
pub mod source_profiler;
pub mod video_plan;

#[allow(unsafe_code)]
pub mod ffi;
