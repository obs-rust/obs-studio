//! Rust ports of the units extracted from `libobs/obs-source.c`,
//! `libobs/obs-source-deinterlace.c` and `libobs/obs-source-transition.c`
//! (one of them with decisions of `libobs/obs-scene.c`),
//! one module per file: so far
//! `libobs/obs-source-activate.c.h`, `obs_source_activate()` and
//! `obs_source_deactivate()` with their tree callbacks (the counters' atomic
//! steps stay in a C fields adapter, the walk in
//! `obs_source_enum_active_tree()`);
//! `libobs/obs-source-async-decoupled.c.h`, the async decoupled setter
//! `obs_source_set_async_decoupled()` (generic over the source; its C shim
//! is in `obs-source-seam`);
//! `libobs/obs-source-async-pacing.c.h`, the async video frame pacing of
//! `get_closest_frame()`; `libobs/obs-source-async-upload.c.h`, the async
//! video upload decisions of `set_async_texture_size()`,
//! `update_async_texrender()`, `update_async_textures()`,
//! `check_to_swap_bgrx_bgra()` and `obs_source_update_async_video()`;
//! `libobs/obs-source-audio-input.c.h`, the audio
//! input steps of `process_audio()` and `reset_resampler()`;
//! `libobs/obs-source-audio-input-path.c.h`, the path that takes one audio packet
//! from a source, `obs_source_output_audio()`;
//! `libobs/obs-source-audio-output.c.h`, the path that puts one audio packet
//! of a source in its buffer, `source_output_audio_data()`;
//! `libobs/obs-source-audio-place.c.h`, the audio placement and action
//! timeline of `source_output_audio_place()`,
//! `source_output_audio_push_back()`, `source_output_audio_data()` and
//! `apply_audio_actions()`;
//! `libobs/obs-source-audio-render.c.h`, the audio render mix plan of
//! `obs_source_audio_render()`, `custom_audio_render()` and
//! `process_audio_source_tick()`;
//! `libobs/obs-source-audio-timing.c.h`, the audio timestamp placement of
//! `source_output_audio_data()`; `libobs/obs-source-color-space.c.h`, the
//! colour space query routing of `obs_source_get_color_space()`;
//! `libobs/obs-source-conversion-plan.c.h`, the async video GPU conversion
//! plan (convert type, planes, technique and linear output);
//! the rest of `libobs/obs-source-deinterlace.c.h`, the deinterlace of an
//! async source (the frame take of `deinterlace_process_last_frame()`, the
//! texture update, the render and the mode and field-order setters,
//! generic over the deinterlace seam and the libobs and graphics calls;
//! its C shims are in `obs-source-seam`);
//! `libobs/obs-source-deinterlace-draw.c.h`, the draw decisions of
//! deinterlacing (the effect, the early return, the linear sRGB flag, the
//! technique and multiplier, the second field and the mode change);
//! `libobs/obs-source-draw.c.h`, the exported draw helpers
//! `obs_source_draw_set_color_matrix()` and `obs_source_draw()`;
//! `libobs/obs-source-deinterlace-pacing.c.h`, the frame pacing of
//! double-rate deinterlacing; `libobs/obs-source-deinterlace-select.c.h`,
//! the frame selection of deinterlacing (which queued frames a deinterlaced
//! source drops and takes, and the texture update);
//! `libobs/obs-source-deinterlace-texture.c.h`, the previous-frame textures
//! of `set_deinterlace_texture_size()` (the format, the texrender and the
//! plane textures, generic over the graphics calls);
//! `libobs/obs-source-draw-plan.c.h`, the source
//! draw decisions of `rotate_async_video()`,
//! `obs_source_render_async_video()` and `source_render()` (the rotation
//! translate, the linear sRGB flag and non-linear alpha, the technique, the
//! multiplier and the colour space conversion);
//! `libobs/obs-source-duplicate.c.h`, the duplicate decisions of
//! `obs_source_duplicate()` and `get_new_filter_name()`, and the scene
//! duplicate decisions of `obs_scene_duplicate()`, `dup_child()` and
//! `dup_group()` in `libobs/obs-scene.c` (the route of a duplicate, the
//! filter numbers, the flags and item routes of a scene copy and the reuse
//! of a repeated source);
//! `libobs/obs-source-duplicate-entry.c.h`, `obs_source_duplicate()` (the
//! field reads and writes, lookups, scene copy, creates, settings calls and
//! filter copies stay in a C fields adapter);
//! `libobs/obs-source-enum-full-tree.c.h`, `obs_source_enum_full_tree()` and
//! the hidden stop-source walk `obs_source_active_child_enum_full_tree()`
//! (the type, callback and data reads and the plugin calls stay in a C
//! fields adapter);
//! `libobs/obs-source-filter-add.c.h`, `obs_source_filter_add()` (the lock,
//! list, reference, signals and callback stay in a C fields adapter);
//! `libobs/obs-source-filter-backup.c.h`, the filter backup of
//! `obs_source_backup_filters()` (each filter saved in list order under
//! the filter lock; the lock and list reads stay in a C fields adapter);
//! `libobs/obs-source-filter-chain.c.h`, the filter chain rules of
//! `filter_compatible()`, `obs_source_filter_add()`,
//! `obs_source_filter_remove_refless()`, `move_filter_dir()` and
//! `obs_source_restore_filters()` (compatibility, the add gates and
//! targets, the moves and the restore match);
//! `libobs/obs-source-filter-remove.c.h`, `obs_source_filter_remove()` and
//! its hidden step `obs_source_filter_remove_refless()` (the lock, signals,
//! callback and release stay in a C fields adapter);
//! `libobs/obs-source-frame-copy.c.h`, the async frame copy plan and cache
//! decisions of `copy_frame_data()`, `cache_video()`, `clean_cache()` and the
//! `obs_source_output_video()` families;
//! `libobs/obs-source-frame-copy-data.c.h`, `obs_source_frame_copy()` and
//! the `copy_frame_data()` it runs (the metadata, the colour range and the
//! plane copies of the plan above);
//! `libobs/obs-source-info-lookup.c.h`, the source type lookup of
//! `get_source_info()` (the first registered type with the asked ID; a NULL
//! ID finds nothing, an intentional difference);
//! `libobs/obs-source-media-admission.c.h`, the media action admission
//! decisions of the `obs_source_media_*()` functions and
//! `process_media_actions()` (whether a call is accepted, the getters'
//! fallbacks, and each queued action's callback and signal);
//! `libobs/obs-source-audio-settings.c.h`, the volume, sync offset, balance,
//! mute and monitoring setters and getters (generic over a seam, shims in
//! `obs-source-seam`);
//! `libobs/obs-source-monitoring.c.h`, the monitoring, monitor-only mute,
//! balance, mixer, flag, push-to and audio active rules of
//! `obs_source_set_monitoring_type()`, `obs_source_set_monitoring_enabled()`,
//! `obs_source_set_balance_value()`, `obs_source_set_audio_mixers()`,
//! `obs_source_set_flags()`, the push-to enables and
//! `obs_source_set_audio_active()`;
//! `libobs/obs-source-output-capture-device.c.h`,
//! `obs_source_audio_output_capture_device_changed()` and its `activate`
//! handler (the monitoring fields, host and platform device calls, signals,
//! log lines and settings reads stay in a C fields adapter);
//! `libobs/obs-source-preload-video-internal.c.h`, the hidden
//! `obs_source_preload_video_internal()` every `obs_source_preload_video*()`
//! entry ends in (`destroying()`, the shapes, the preload frame's create,
//! destroy and copy, the mutex and the `last_frame_ts` write stay in a C
//! fields adapter);
//! `libobs/obs-source-process-filter-begin.c.h`,
//! `obs_source_process_filter_begin()` and
//! `obs_source_process_filter_begin_with_color_space()` (the fields, the
//! target and parent lookups, the base size, the color space, the skip,
//! the texrender and the draw stay in a C fields adapter);
//! `libobs/obs-source-process-filter-end.c.h`,
//! `obs_source_process_filter_end()` and
//! `obs_source_process_filter_tech_end()` (the bypass flag, the target and
//! parent lookups, the flags read, the texture lookup and the draws stay in
//! a C fields adapter);
//! `libobs/obs-source-ptm-gate.c.h`, the
//! push-to-mute and push-to-talk gate of `source_muted()`;
//! `libobs/obs-source-push-to-enable.c.h`, the push-to-mute and push-to-talk
//! enable setters (`obs_source_enable_push_to_mute()` and
//! `obs_source_enable_push_to_talk()`, generic over the source, under its
//! `audio_mutex`);
//! `libobs/obs-source-refs.c.h`, the strong-reference entries
//! `obs_source_addref()`, `obs_source_release()`, `obs_source_get_ref()` and
//! `obs_weak_source_get_source()` (the control, the core check and the
//! destroy stay in a C fields adapter);
//! `libobs/obs-source-remove.c.h`, `obs_source_remove()` (the removed
//! flag, reference, signals, canvas removal and release stay in a C fields
//! adapter);
//! `libobs/obs-source-render-route.c.h`, the source and filter render
//! routing (which draw a source or a filter takes, the filter bypass, the
//! default effect and the linear sRGB flags);
//! `libobs/obs-source-restore-filters.c.h`, `obs_source_restore_filters()`
//! (the lock, list, array reads, references, loads and releases stay in a C
//! fields adapter);
//! `libobs/obs-source-set-name.c.h`, the source rename
//! `obs_source_set_name()` (the unchanged-name check, the canvas or table
//! rename, and its signals);
//! `libobs/obs-source-set-video-frame-internal.c.h`, the hidden
//! `obs_source_set_video_frame_internal()` every
//! `obs_source_set_video_frame*()` entry ends in (the graphics context, the
//! preload frame's shapes, create, destroy and copy, the async texture size
//! and update, the stale flag, the mutex and the `last_frame_ts` write stay
//! in C fields adapters);
//! `libobs/obs-source-size.c.h`, the size query routing of
//! `obs_source_get_width()` and `obs_source_get_height()`;
//! `libobs/obs-source-skip-video-filter.c.h`,
//! `obs_source_skip_video_filter()` (the target and parent lookups, the
//! field reads and the draws stay in a C fields adapter);
//! `libobs/obs-source-transition-access.c.h`, the getters and manual
//! controls of the transitions (`obs_transition_get_source()`,
//! `obs_transition_get_active_source()`, `obs_transition_set_manual_torque()`
//! and `obs_transition_set_manual_time()`, generic over the transition seam
//! and the libobs calls; its C shims are in `obs-source-seam`);
//! `libobs/obs-source-transition-clock.c.h`, the clock and fit decisions of
//! the transitions; `libobs/obs-source-transition-load.c.h`, the type
//! check, the scale type rule, the load's source steps and the swap checks
//! of the transitions (`transition_valid()`, `obs_transition_load()`,
//! `copy_source_state()` and the swap gate), with
//! `libobs/obs-source-transition-restore.c.h`, `obs_transition_load()`
//! itself (generic over the transition seam, the `obs_data` reads and the
//! libobs source calls; its C shim is in `obs-source-seam`);
//! `libobs/obs-source-transition-render.c.h`, the render
//! and audio step decisions of the transitions;
//! `libobs/obs-source-transition-sample-time.c.h`, the time of each audio
//! sample a transition mixes (`get_sample_time()`);
//! `libobs/obs-source-transition-settings.c.h`, the settings of the
//! transitions (`obs_transition_is_active()`, the scale type, alignment and
//! size setters and getters, `obs_transition_save()` and the fixed duration
//! switch, generic over the transition seam; its C shims are in the
//! `obs-source-seam` crate, which only libobs-rust links);
//! `libobs/obs-source-transition-size.c.h`, the size, child matrices and
//! tick of the transitions (`transition_get_cx()`, `transition_get_cy()`,
//! `transition_recalculate_matrices()`, `transition_recalculate_size()` and
//! `obs_transition_tick()`, generic over the transition seam and the libobs
//! calls; its C shims are in `obs-source-seam` too);
//! `libobs/obs-source-transition-slots.c.h`, the two child slots of the
//! transitions (`obs_transition_clear()`, `obs_transition_enum_sources()`
//! and the swap of `obs_transition_swap_begin()`, `copy_source_state()` and
//! `obs_transition_swap_end()`, generic over the transition seam and the
//! libobs source calls; its C shims are in `obs-source-seam` too);
//! `libobs/obs-source-transition-start.c.h`, the start checks and child
//! handover of the transitions (`obs_transition_start()`, `set_source()`,
//! `activate_transition()`, the active source, the manual reset and the
//! active child rule of `obs_transition_set()` and the swap);
//! `libobs/obs-source-transition-stop.c.h`, the clock read and stop of the
//! transitions (`obs_transition_get_time()`, `obs_transition_force_stop()`
//! and the render helpers `transition_calc_time()`,
//! `transition_get_video_time()`, `transition_stop()` and
//! `transition_handle_stop()`, generic over the transition seam and the
//! libobs calls; its C shims are in `obs-source-seam` too);
//! the rest of `libobs/obs-source-transition.c.h`, the audio render, init
//! and free of the transitions (`obs_transition_audio_render()`,
//! `obs_transition_init()` and `obs_transition_free()`, generic over the
//! transition seam, the audio buffers and the libobs calls; its C shims are
//! in `obs-source-seam` too);
//! `libobs/obs-source-transition-switch.c.h`, the start and set of the
//! transitions (`obs_transition_start()` and `obs_transition_set()`, with
//! `set_source()`, `activate_transition()`, `write_start_clock()` and the
//! recursion check, generic over the transition seam and the libobs calls;
//! its C shims are in `obs-source-seam` too);
//! `libobs/obs-source-transition-video.c.h`, the video render of the
//! transitions (`obs_transition_video_render()`, `_render2()`,
//! `_get_color_space()` and `_render_direct()`, with `get_texture()`,
//! `copy_transition_state()`, `render_child()` and `mix_spaces()`, generic
//! over the transition seam, the libobs calls and the graphics calls; its
//! C shims are in `obs-source-seam` too);
//! `libobs/obs-source-video-tick.c.h`, the per-frame decisions of
//! `obs_source_video_tick()` and `async_tick()` (which steps run, the
//! deferred update, the show and activate edges, the plugin callback and
//! the async route);
//! `libobs/obs-source-video-tick-callbacks.c.h`, the per-source callback
//! steps of `obs_source_video_tick()` (`obs_source_deferred_update()`,
//! `activate_source()`, `deactivate_source()`, `show_source()` and
//! `hide_source()`: the callback checks and calls, the deferred count and
//! `obs_source_dosignal()` stay in C fields adapters);
//! `libobs/obs-source-volume.c.h`, the volume snap of
//! `get_source_volume()` and the apply decisions of `apply_audio_volume()`;
//! and `libobs/obs-source-volume-mul.c.h`, the volume multiply kernels of
//! `multiply_output_audio()` and `multiply_vol_data()`.
//!
//! The safe cores are [`async_pacing`], [`async_upload`], [`audio_input`],
//! [`audio_input_path`],
//! [`audio_output`], [`audio_place`],
//! [`audio_render_plan`], [`audio_timing`],
//! [`color_space`], [`conversion_plan`], [`deinterlace`], [`deinterlace_draw`],
//! [`deinterlace_pacing`], [`deinterlace_select`], [`draw_plan`], [`source_draw`], [`filter_backup`], [`frame_copy`], [`media_admission`], [`monitoring`], [`own_clock_drift`] (new code with no C original, #4182), [`ptm_gate`], [`render_route`], [`set_name`], [`size_route`], [`source_duplicate`], [`source_filter_chain`],
//! [`transition_access`], [`transition_audio`], [`transition_clock`], [`transition_load`], [`transition_render`],
//! [`transition_sample_time`], [`transition_settings`], [`transition_size`],
//! [`transition_slots`],
//! [`transition_start`], [`transition_stop`], [`transition_switch`],
//! [`transition_video`],
//! [`video_tick`], [`video_tick_callbacks`], [`volume_apply`] and
//! [`volume_mul`]; [`draw_technique`] holds the
//! technique and colour space values [`deinterlace_draw`] and [`draw_plan`]
//! share. The C
//! ABI shims live in [`ffi`].

#![deny(unsafe_code)]

pub mod activate;
pub mod active_child;
pub mod async_compensation;
pub mod async_decoupled;
pub mod async_flag_getters;
pub mod async_pacing;
pub mod async_rotation;
pub mod async_upload;
pub mod audio_active;
pub mod audio_active_setter;
pub mod audio_buffer_getters;
pub mod audio_capture_callbacks;
pub mod audio_input;
pub mod audio_input_path;
pub mod audio_mixers_getter;
pub mod audio_output;
pub mod audio_pause_callbacks;
pub mod audio_pending_speaker_layout;
pub mod audio_place;
pub mod audio_render_plan;
pub mod audio_settings;
pub mod audio_timing;
pub mod base_size;
pub mod boolean_queries;
pub mod callback_registry;
pub mod canvas_getter;
pub mod caption_callbacks;
pub mod color_space;
pub mod configurable;
pub mod conversion_plan;
pub mod copy_filters;
pub mod default_flags;
pub mod defaults;
pub mod deinterlace;
pub mod deinterlace_draw;
pub mod deinterlace_getters;
pub mod deinterlace_pacing;
pub mod deinterlace_select;
pub mod deinterlace_texture;
pub mod draw_plan;
pub mod draw_technique;
pub mod enabled_setter;
pub mod enum_filters;
pub mod enum_full_tree;
pub mod filter_add;
pub mod filter_backup;
pub mod filter_count;
pub mod filter_index;
pub mod filter_relationship_getters;
pub mod filter_remove;
pub mod filter_set_order;
pub mod flags_mixers;
pub mod frame_copy;
pub mod frame_copy_data;
pub mod frame_init;
pub mod frame_wrappers;
pub mod get_release_frame;
pub mod get_weak;
pub mod handler_getters;
pub mod hidden_setter;
pub mod info_lookup;
pub mod init_context;
pub mod interaction;
pub mod media_admission;
pub mod media_controls;
pub mod missing_files_getter;
pub mod monitoring;
pub mod mute_monitor_hotkeys;
pub mod muted_volume;
pub mod output_capture_device;
pub mod output_cea708;
pub mod output_flags;
pub mod own_clock_drift;
pub mod preload_video_internal;
pub mod private_settings;
pub mod process_filter_begin;
pub mod process_filter_end;
pub mod properties;
pub mod ptm_gate;
pub mod push_delay;
pub mod push_hotkeys;
pub mod push_queries;
pub mod push_to_enable;
pub mod removed;
pub mod render_route;
pub mod replace_missing_file;
pub mod restore_filters;
pub mod save_load;
pub mod set_name;
pub mod set_video_frame_internal;
pub mod settings_getter;
pub mod show_active_counters;
pub mod show_preloaded_video;
pub mod showing_active;
pub mod size_route;
pub mod skip_video_filter;
pub mod source_draw;
pub mod source_duplicate;
pub mod source_duplicate_entry;
pub mod source_filter_chain;
pub mod source_icon_type;
pub mod source_identity_getters;
pub mod source_name;
pub mod source_remove;
pub mod state_getters;
pub mod strong_refs;
pub mod theme_icon;
pub mod transition_access;
pub mod transition_audio;
pub mod transition_clock;
pub mod transition_load;
pub mod transition_render;
pub mod transition_sample_time;
pub mod transition_settings;
pub mod transition_size;
pub mod transition_slots;
pub mod transition_start;
pub mod transition_stop;
pub mod transition_switch;
pub mod transition_video;
pub mod type_data;
pub mod type_info;
pub mod update;
pub mod update_properties;
pub mod video_tick;
pub mod video_tick_callbacks;
pub mod volume_apply;
pub mod volume_mul;
pub mod weak_predicates;
pub mod weak_refs;

mod platform;

#[allow(unsafe_code)]
pub mod ffi;

pub mod type_id_getters;

pub mod media_getters;

pub mod module_lookup;
