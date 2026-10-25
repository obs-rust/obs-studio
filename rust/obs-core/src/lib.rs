//! Rust ports of the units extracted from the libobs core, one module per
//! file: so far `libobs/obs-video-plan.c.h` and `libobs/obs-context-names.c.h`
//! (from `obs.c.h`), `libobs/obs-module-paths.c.h`,
//! `libobs/obs-module-names.c.h`, `libobs/obs-module-locale.c.h`,
//! `libobs/obs-module-source-rules.c.h` and `libobs/obs-module-type-rules.c.h`
//! (from `obs-module.c.h`) and `libobs/obs-audio-mix.c.h` (from
//! `obs-audio.c.h`); and
//! `libobs/util/source-profiler.c.h`, which reads core state (sources and the
//! graphics context) and so belongs to the core rather than obs-util; and
//! `libobs/obs-view-api.c.h`, the views; `libobs/obs-video-copy-plan.c.h`
//! and `libobs/obs-video-output-step.c.h` (from `obs-video.c.h`), the output
//! frame copy plan and scale effect choice, and the per-frame video output
//! step decisions; and `libobs/obs-source-load.c.h` (from `obs.c.h`), the
//! source load migration and value rules; and `libobs/obs-canvas-draw.c.h`
//! (from `obs.c.h` and `obs-display.c.h`), the canvas texture draw and
//! display clear decisions; and `libobs/obs-audio-discard.c.h` (from
//! `obs-audio.c.h`), the steps that drop audio from a source's input; and
//! `libobs/obs-audio-min-ts.c.h` (from `obs-audio.c.h`), the per-source
//! decisions of the minimum audio timestamp step.
//!
//! The safe cores are [`video_plan`], [`context_names`], [`module_paths`],
//! [`module_names`], [`module_locale`], [`module_source_rules`],
//! [`module_type_rules`], [`audio_mix`], [`source_profiler`], [`view`],
//! [`video_copy_plan`], [`video_output_step`], [`source_load`],
//! [`canvas_draw`], [`audio_discard`] and [`audio_min_ts`].
//! [`video_plan`] sizes the conversion textures; [`video_output_step`]
//! decides, each frame, what is rendered, converted, staged, queued for the
//! GPU encoders and downloaded; [`video_copy_plan`] plans the copies of a
//! downloaded frame into each output frame; [`canvas_draw`] picks the
//! technique a canvas texture draws into a display with, and how the display
//! is cleared. The C ABI shims live in [`ffi`].

#![deny(unsafe_code)]

pub mod audio_discard;
pub mod audio_min_ts;
pub mod audio_mix;
pub mod canvas_draw;
pub mod context_names;
pub mod module_locale;
pub mod module_names;
pub mod module_paths;
pub mod module_source_rules;
pub mod module_type_rules;
mod platform;
pub mod source_load;
pub mod source_profiler;
pub mod video_copy_plan;
pub mod video_output_step;
pub mod video_plan;
pub mod view;

#[allow(unsafe_code)]
pub mod ffi;
