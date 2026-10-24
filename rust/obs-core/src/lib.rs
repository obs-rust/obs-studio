//! Rust ports of the units extracted from the libobs core, one module per
//! file: so far `libobs/obs-video-plan.c.h` and `libobs/obs-context-names.c.h`
//! (from `obs.c.h`), `libobs/obs-module-paths.c.h`,
//! `libobs/obs-module-names.c.h`, `libobs/obs-module-locale.c.h` and
//! `libobs/obs-module-source-rules.c.h` (from `obs-module.c.h`) and
//! `libobs/obs-audio-mix.c.h` (from `obs-audio.c.h`); and
//! `libobs/util/source-profiler.c.h`, which reads core state (sources and the
//! graphics context) and so belongs to the core rather than obs-util; and
//! `libobs/obs-view-api.c.h`, the views; `libobs/obs-video-copy-plan.c.h`
//! (from `obs-video.c.h`), the output frame copy plan and scale effect choice;
//! and `libobs/obs-source-load.c.h` (from `obs.c.h`), the source load
//! migration and value rules.
//!
//! The safe cores are [`video_plan`], [`context_names`], [`module_paths`],
//! [`module_names`], [`module_locale`], [`module_source_rules`], [`audio_mix`],
//! [`source_profiler`], [`view`], [`video_copy_plan`] and [`source_load`].
//! [`video_plan`] sizes the conversion textures; [`video_copy_plan`] plans
//! the copies out of them into each output frame. The C ABI shims live in
//! [`ffi`].

#![deny(unsafe_code)]

pub mod audio_mix;
pub mod context_names;
pub mod module_locale;
pub mod module_names;
pub mod module_paths;
pub mod module_source_rules;
mod platform;
pub mod source_load;
pub mod source_profiler;
pub mod video_copy_plan;
pub mod video_plan;
pub mod view;

#[allow(unsafe_code)]
pub mod ffi;
