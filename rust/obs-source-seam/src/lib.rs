//! The C ABI of the `obs-source` ports that call back into libobs: so far
//! `libobs/obs-source-transition-settings.c.h`, the transition settings,
//! whose shims ([`ffi::source_transition_settings`]) implement the safe
//! core's `obs_source::transition_settings::Transition` over the transition
//! seam (`libobs/obs-source-transition-seam.h`, hidden C in libobs) and its
//! `SaveData` over `obs_data_set_string()` and `obs_data_set_int()`;
//! `libobs/obs-source-transition-size.c.h`, the size, child matrices and tick
//! of a transition, whose shims ([`ffi::source_transition_size`]) implement
//! `obs_source::transition_size::Transition` over the same seam and the
//! libobs calls (`obs_source_get_width()`, `obs_source_get_height()`,
//! `gs_texrender_reset()`, `add_alignment()`, `calc_torquef()` and the
//! matrix calls); `libobs/obs-source-transition-restore.c.h`, the load of a
//! saved transition, whose shim ([`ffi::source_transition_restore`])
//! implements `obs_source::transition_load::Transition` on the sizing's
//! `Seam`, over the same seam, `obs_data_get_string()`, `obs_data_get_int()`
//! and the libobs source calls (`obs_get_source_by_name()`, the active-child
//! add and remove, `obs_source_release()` and `blog()`);
//! `libobs/obs-source-transition-slots.c.h`, the child slots of a
//! transition, whose shims ([`ffi::source_transition_slots`]) implement
//! `obs_source::transition_slots::Transition` over the same seam and the
//! libobs source calls (`obs_source_get_ref()`, `obs_source_release()`, the
//! active-child add and remove, `obs_source_get_name()` and `blog()`);
//! `libobs/obs-source-transition-access.c.h`, the getters and manual
//! controls of a transition, whose shims
//! ([`ffi::source_transition_access`]) implement
//! `obs_source::transition_access::Transition` over the same seam,
//! `obs_source_get_ref()` and `obs_transition_set()`;
//! `libobs/obs-source-transition-stop.c.h`, the clock read and stop of a
//! transition, whose shims ([`ffi::source_transition_stop`]) implement
//! `obs_source::transition_stop::Transition` over the same seam (with the
//! plugin's `transition_stop`, the signals and the video frame time
//! through it), `obs_source_remove_active_child()` and
//! `obs_source_release()`;
//! `libobs/obs-source-transition-switch.c.h`, the start and set of a
//! transition, whose shims ([`ffi::source_transition_switch`]) implement
//! `obs_source::transition_switch::Transition` over the same seam (with the
//! plugin's `transition_start` and the signals through it), the libobs
//! source calls (`obs_source_get_ref()`, `obs_source_release()`, the
//! active-child add and remove, `obs_source_get_name()`,
//! `obs_source_enum_full_tree()` and `blog()`), `os_gettime_ns()` and the
//! size port's recalculation ([`ffi::source_transition_size`]); and
//! `libobs/obs-source-transition-video.c.h`, the video render of a
//! transition, whose shims ([`ffi::source_transition_video`]) implement
//! `obs_source::transition_video::Transition` over the same seam (with the
//! matrices, the texrenders, both locks and the context data the draw
//! callback is handed through it), the stop and size ports
//! ([`ffi::source_transition_stop`], [`ffi::source_transition_size`]), the
//! libobs source calls (`obs_source_active()`, `obs_source_get_ref()`,
//! `obs_source_release()`, `obs_source_get_color_space()` and
//! `obs_source_video_render()`), the signal, the transparent texture and
//! the graphics calls. The rest
//! of `libobs/obs-source-transition.c.h`, the audio render, init and free
//! of a transition, has its shims in [`ffi::source_transition`], which
//! implement `obs_source::transition_audio::Transition` on the stop shims'
//! `Seam`, over the same seam and the libobs source, audio and graphics
//! calls (`obs_source_removed()`, the audio timestamp and mix reads,
//! `obs_source_get_ref()`, the texrender calls and the graphics context).
//!
//! `libobs/obs-source-audio-settings.c.h`, the source audio settings, have
//! their shims in [`ffi::source_audio_settings`], over
//! `libobs/obs-source-audio-settings-seam.h`.
//!
//! `libobs/obs-source-audio-input-path.c.h`, the path that takes one audio packet
//! from a source, has its shim in [`ffi::source_audio_input_path`], over
//! `libobs/obs-source-audio-input-path-seam.h`.
//!
//! `libobs/obs-source-audio-output.c.h`, the path that puts one audio packet in
//! a source's buffer, has its shims in [`ffi::source_audio_output`], over
//! `libobs/obs-source-audio-output-seam.h`.
//!
//! The rest of `libobs/obs-source-deinterlace.c.h`, the deinterlace of an
//! async source (the frame take, the texture update, the render and the
//! mode and field-order setters), has its shims in
//! [`ffi::source_deinterlace`], which implement
//! `obs_source::deinterlace::Source` over
//! `libobs/obs-source-deinterlace-seam.h` and
//! `obs_source_deinterlace_fields()`, with the frame cache, the async
//! texture upload, the effect load and the graphics calls through its
//! `Calls`.
//!
//! These shims live apart from `obs-source` because they reference symbols
//! only libobs has. The `*_steps_rust` C tests link libobs and the Rust
//! staticlib alone and pull in the `obs-source` objects they test; a shim
//! sharing those objects would make them pull in the hidden seam and, from
//! the `obs_data` calls, most of the staticlib and its system libraries.
//! Only libobs-rust links this crate, so no such test ever loads it.

#![deny(unsafe_code)]

#[allow(unsafe_code)]
pub mod ffi;
