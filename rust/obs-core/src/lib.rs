//! Rust ports of the units extracted from the libobs core, one module per
//! file: so far `libobs/obs-video-plan.c.h`, `libobs/obs-context-names.c.h`
//! and `libobs/obs-core-setup-rules.c.h` (from `obs.c.h`), `libobs/obs-module-paths.c.h`,
//! `libobs/obs-module-names.c.h`, `libobs/obs-module-locale.c.h`,
//! `libobs/obs-module-source-rules.c.h`, `libobs/obs-module-type-rules.c.h`,
//! `libobs/obs-module-load-rules.c.h` and
//! `libobs/obs-module-register-copy.c.h` (from `obs-module.c.h`) and `libobs/obs-audio-mix.c.h` (from
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
//! decisions of the minimum audio timestamp step; and
//! `libobs/obs-canvas-rules.c.h` (from `obs-canvas.c.h`), the canvas flag
//! rules and setter checks; `libobs/obs-video-pacing.c.h` (from
//! `obs-video.c.h`), the frame pacing decisions of `video_sleep()`;
//! `libobs/obs-video-frame-timing.c.h` (from `obs-video.c.h`), the tick time
//! and frame statistics of the graphics thread loop; and
//! `libobs/obs-nix-system-info.c.h` (from `obs-nix.c.h`), the Linux and
//! BSD system info parsing of the startup log; and
//! `libobs/obs-win-crash-report.c.h` (from `obs-win-crash-handler.c.h`),
//! the Windows crash report rules.
//!
//! The safe cores are [`video_plan`], [`context_names`], [`core_setup_rules`],
//! [`module_paths`],
//! [`module_names`], [`module_locale`], [`module_source_rules`],
//! [`module_type_rules`], [`module_load_rules`], [`module_register_copy`],
//! [`audio_mix`],
//! [`source_profiler`], [`view`], [`video_copy_plan`], [`video_output_step`],
//! [`source_load`],
//! [`canvas_draw`], [`canvas_rules`], [`audio_discard`], [`audio_min_ts`],
//! [`audio_render`], [`video_pacing`], [`video_frame_timing`],
//! [`nix_system_info`] and [`win_crash_report`].
//! [`video_plan`] sizes the conversion textures; [`video_output_step`]
//! decides, each frame, what is rendered, converted, staged, queued for the
//! GPU encoders and downloaded; [`video_copy_plan`] plans the copies of a
//! downloaded frame into each output frame; [`canvas_draw`] picks the
//! technique a canvas texture draws into a display with, and how the display
//! is cleared; [`canvas_rules`] holds the flags a canvas is created with and
//! the checks of its setters; [`video_pacing`] steps the frame time after the
//! sleep at the end of each frame; [`video_frame_timing`] gives the sources
//! their tick time and sums the frame statistics; [`nix_system_info`] parses
//! the `/proc/cpuinfo` and `dmesg.boot` lines and the memory sizes the
//! startup log prints; [`win_crash_report`] decides which module holds the
//! crash address, which threads the report traces and in what order, how
//! each frame is written and when the crash handler passes on; [`module_load_rules`] decides which modules load, are
//! disabled or skipped, and which failures count; [`module_register_copy`]
//! decides how much of a plugin's registration structure is copied and
//! which of its fields lie within the size it gave; [`core_setup_rules`]
//! holds the video and audio reset gates, the audio buffering ticks, the
//! command line copy, the data file search, the data path and latest input
//! type rules, the monitoring device checks and which mixes count toward
//! the frame counts.
//! The C ABI shims live in [`ffi`].

#![deny(unsafe_code)]

pub mod audio_discard;
pub mod audio_min_ts;
pub mod audio_mix;
pub mod audio_render;
pub mod canvas_draw;
pub mod canvas_rules;
pub mod context_names;
pub mod core_setup_rules;
pub mod module_load_rules;
pub mod module_locale;
pub mod module_names;
pub mod module_paths;
pub mod module_register_copy;
pub mod module_source_rules;
pub mod module_type_rules;
pub mod nix_system_info;
mod platform;
pub mod source_load;
pub mod source_profiler;
pub mod video_copy_plan;
pub mod video_frame_timing;
pub mod video_output_step;
pub mod video_pacing;
pub mod video_plan;
pub mod view;
pub mod win_crash_report;

#[allow(unsafe_code)]
pub mod ffi;
