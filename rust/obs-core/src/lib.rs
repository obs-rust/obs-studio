//! Rust ports of the units extracted from the libobs core, one module per
//! file: so far `libobs/obs-video-plan.c.h`, `libobs/obs-context-names.c.h`
//! and `libobs/obs-core-setup-rules.c.h`,
//! `libobs/obs-core-setup-rules-ms.c.h` and
//! `libobs/obs-core-setup-rules-task.c.h` (from `obs.c.h`), and
//! `libobs/obs-shutdown-detach.c.h` (from `obs.c.h`), shutdown's
//! detachment of the items of scenes the host left alive,
//! `libobs/obs-module-paths.c.h`,
//! `libobs/obs-module-names.c.h`, `libobs/obs-module-lists.c.h` (over
//! `libobs/obs-module-lists-seam.h`), `libobs/obs-module-locale.c.h`,
//! `libobs/obs-module-source-rules.c.h`, `libobs/obs-module-type-rules.c.h`,
//! `libobs/obs-module-load-rules.c.h` with
//! `libobs/obs-module-load-rules-skip.c.h`, and
//! `libobs/obs-module-register-copy.c.h` (from `obs-module.c.h`) and `libobs/obs-audio-mix.c.h` (from
//! `obs-audio.c.h`); and
//! `libobs/util/source-profiler.c.h`, which reads core state (sources and the
//! graphics context) and so belongs to the core rather than obs-util; and
//! `libobs/obs-view-api.c.h`, the views; `libobs/obs-video-copy-plan.c.h`,
//! `libobs/obs-video-frame-copy.c.h` and `libobs/obs-video-output-step.c.h`
//! (from `obs-video.c.h`), the output frame copy plan and scale effect
//! choice, the output frame copy, and the per-frame video output step
//! decisions; and `libobs/obs-source-load.c.h` (from `obs.c.h`), the
//! source load migration and value rules; and `libobs/obs-canvas-draw.c.h`
//! (from `obs.c.h` and `obs-display.c.h`), the canvas texture draw and
//! display clear decisions; and `libobs/obs-audio-discard.c.h` (from
//! `obs-audio.c.h`), the steps that drop audio from a source's input; and
//! `libobs/obs-audio-min-ts.c.h` (from `obs-audio.c.h`), the per-source
//! decisions of the minimum audio timestamp step; and
//! `libobs/obs-canvas-rules.c.h` (from `obs-canvas.c.h`), the canvas flag
//! rules and setter checks; `libobs/obs-video-pacing.c.h` (from
//! `obs-video.c.h`), the frame pacing decisions of `video_sleep()`;
//! `libobs/obs-video-stop-requested.c.h` (from `obs-video.c.h`), the graphics
//! thread's check that every mix's output has stopped;
//! `libobs/obs-video-unmap-last-surface.c.h` (from `obs-video.c.h`), the
//! staging cleanup at the start of each output stage;
//! `libobs/obs-video-graphics-tasks.c.h` (from `obs-video.c.h`), the
//! graphics thread's drain of its task queue;
//! `libobs/obs-video-frame-timing.c.h` (from `obs-video.c.h`), the tick time
//! and frame statistics of the graphics thread loop; and
//! `libobs/obs-nix-system-info.c.h` (from `obs-nix.c.h`), the Linux and
//! BSD system info parsing of the startup log; and
//! `libobs/obs-nix-log-rules.c.h` (from `obs-nix.c.h` and
//! `obs-nix-os-release.h`), its distribution, Flatpak and desktop log
//! rules; and
//! `libobs/obs-win-crash-report.c.h` (from `obs-win-crash-handler.c.h`),
//! the Windows crash report rules; and `libobs/obs-windows-symbol-paths.c.h`
//! and `libobs/obs-windows-system-info.c.h` (from `obs-windows.c.h`), the
//! Windows debug symbol search path and system info log rules; and
//! `libobs/obs-audio.c.h` itself, the audio tick (`audio_callback()`), over
//! the core-audio seam; and `libobs/obs-display.c.h` itself, the display
//! (create, destroy, resize, the draw callbacks and `render_display()`),
//! over the display seam.
//!
//! The safe cores are [`video_plan`], [`context_names`], [`core_setup_rules`],
//! [`module_paths`],
//! [`module_names`], [`module_lists`], [`module_locale`], [`module_source_rules`],
//! [`module_type_rules`], [`module_load_rules`], [`module_register_copy`],
//! [`audio_mix`], [`audio_callback`], [`display`],
//! [`source_profiler`], [`view`], [`video_copy_plan`], [`video_frame_copy`],
//! [`video_output_step`],
//! [`source_load`],
//! [`canvas_draw`], [`canvas_rules`], [`audio_discard`], [`audio_min_ts`],
//! [`audio_render`], [`video_pacing`], [`video_stop_requested`],
//! [`video_unmap_last_surface`],
//! [`video_graphics_tasks`],
//! [`video_frame_timing`],
//! [`shutdown_detach`],
//! [`nix_system_info`], [`win_crash_report`], [`win_symbol_paths`] and
//! [`win_system_info`].
//! [`video_plan`] sizes the conversion textures; [`video_output_step`]
//! decides, each frame, what is rendered, converted, staged, queued for the
//! GPU encoders and downloaded; [`video_copy_plan`] plans the copies of a
//! downloaded frame into each output frame, and [`video_frame_copy`] runs
//! them; [`canvas_draw`] picks the
//! technique a canvas texture draws into a display with, and how the display
//! is cleared; [`canvas_rules`] holds the flags a canvas is created with and
//! the checks of its setters; [`video_pacing`] steps the frame time after the
//! sleep at the end of each frame; [`video_stop_requested`] tells the
//! graphics thread when every mix's output has stopped;
//! [`video_unmap_last_surface`] unmaps the staging surfaces the last frame's
//! download left mapped; [`video_graphics_tasks`]
//! runs the tasks queued for the graphics thread, each once and oldest
//! first, with the queue unlocked while each one runs; [`shutdown_detach`] removes, at shutdown, the items of every scene and
//! group still in the source table, holding every one of them until all
//! items are gone, then waits for the destroys that queued; [`video_frame_timing`] gives the sources
//! their tick time and sums the frame statistics; [`nix_system_info`] parses
//! the `/proc/cpuinfo` and `dmesg.boot` lines and the memory sizes the
//! startup log prints, and reads the `/etc/os-release` keys and values, the
//! Flatpak extension names and the desktop line form it logs; [`win_crash_report`] decides which module holds the
//! crash address, which threads the report traces and in what order, how
//! each frame is written and when the crash handler passes on;
//! [`win_symbol_paths`] cuts each module's directory, checks it against the
//! listed ones and joins the DbgHelp search path;
//! [`win_system_info`] packs the Windows version, sizes the memory lines,
//! gates the gaming features and picks each feature's line and the security
//! product labels of the Windows startup log; [`module_load_rules`] decides which modules load, are
//! disabled or skipped (the skip ladder `load_all_callback()` walks before it
//! opens one), and which failures count; [`module_lists`] runs the module
//! list calls (the loaded module log, the type, search path, safe and
//! disabled lists, and a module's file and config paths) over the module
//! list seam; [`module_register_copy`]
//! decides how much of a plugin's registration structure is copied and
//! which of its fields lie within the size it gave; [`core_setup_rules`]
//! holds the video and audio reset gates, the audio buffering ticks and
//! their milliseconds, the command line copy, the data file search, the data path and latest input
//! type rules, the monitoring device checks, which mixes count toward
//! the frame counts, the task queue's route and the video level checks.
//! [`audio_callback`] runs one audio tick: the render order, the render,
//! the buffering, the mix, the discard and the audio tasks, in the C's
//! order and under its locks.
//! [`display`] makes, links, renders and destroys a display, in the C's
//! order and under its locks.
//! The C ABI shims live in [`ffi`].

#![deny(unsafe_code)]

pub mod audio_callback;
pub mod audio_discard;
pub mod audio_info_queries;
pub mod audio_min_ts;
pub mod audio_mix;
pub mod audio_monitoring_device;
pub mod audio_render;
pub mod base_effect;
pub mod canvas_channel_getter;
pub mod canvas_clear_mix;
pub mod canvas_draw;
pub mod canvas_flags_getter;
pub mod canvas_move;
pub mod canvas_readonly_getters;
pub mod canvas_ref;
pub mod canvas_release;
pub mod canvas_render;
pub mod canvas_rules;
pub mod canvas_scene;
pub mod canvas_set_name;
pub mod canvas_signals;
pub mod canvas_valid_video_info;
pub mod canvas_video_getter;
pub mod canvas_video_info_getter;
pub mod context_names;
pub mod core_cmdline_getter;
pub mod core_frame_interval;
pub mod core_initialized;
pub mod core_setup_rules;
pub mod core_texture_activity;
pub mod display;
pub mod display_size;
pub mod display_state;
pub mod enter_leave_graphics;
pub mod enum_contexts;
pub mod enum_modules;
pub mod enum_scenes;
pub mod enum_sources;
pub mod find_modules;
pub mod get_audio;
pub mod get_main_canvas;
pub mod get_main_texture;
pub mod get_output_source;
pub mod get_video;
pub mod get_video_frame_counts;
pub mod global_getters;
pub mod hotkeys_background_status;
pub mod module_disable_query;
pub mod module_failure_info;
pub mod module_init;
pub mod module_lists;
pub mod module_load_all;
pub mod module_load_locale;
pub mod module_load_metadata;
pub mod module_load_rules;
pub mod module_locale;
pub mod module_locale_getters;
pub mod module_metadata;
pub mod module_name_lookups;
pub mod module_names;
pub mod module_paths;
pub mod module_post_load;
pub mod module_register_copy;
pub mod module_source_rules;
pub mod module_type_queries;
pub mod module_type_rules;
pub mod monitoring_device_getter;
pub mod nix_system_info;
pub mod object_id;
pub mod object_queries;
pub mod object_ref;
pub mod object_validity_query;
pub mod output_protocol_registry;
pub mod platform;
pub mod private_data;
pub mod raw_audio_add;
pub mod raw_audio_remove;
pub mod raw_video_callbacks;
pub mod render_canvas_texture;
pub mod reset_audio;
pub mod reset_audio2;
pub mod reset_source_uuids;
pub mod save_sources;
pub mod set_cmdline_args;
pub mod set_locale;
pub mod set_output_source;
pub mod set_video_levels;
pub mod shutdown_detach;
pub mod source_load;
pub mod source_profiler;
pub mod source_type_enum;
pub mod task_queue;
pub mod tick_callbacks;
pub mod type_catalog_enum;
pub mod ui_task_handler;
pub mod version;
pub mod video_active;
pub mod video_copy_plan;
pub mod video_frame_copy;
pub mod video_frame_timing;
pub mod video_graphics_tasks;
pub mod video_info_query;
pub mod video_level_queries;
pub mod video_output_step;
pub mod video_pacing;
pub mod video_plan;
pub mod video_render_passes;
pub mod video_render_size;
pub mod video_scale_effects;
pub mod video_stats_queries;
pub mod video_stop_requested;
pub mod video_unmap_last_surface;
pub mod view;
pub mod weak_object_expired;
pub mod weak_object_identity;
pub mod weak_object_refs;
pub mod win_crash_report;
pub mod win_symbol_paths;
pub mod win_system_info;

#[allow(unsafe_code)]
pub mod ffi;
