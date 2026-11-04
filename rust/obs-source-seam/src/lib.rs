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
//! add and remove, `obs_source_release()` and `blog()`); and
//! `libobs/obs-source-transition-slots.c.h`, the child slots of a
//! transition, whose shims ([`ffi::source_transition_slots`]) implement
//! `obs_source::transition_slots::Transition` over the same seam and the
//! libobs source calls (`obs_source_get_ref()`, `obs_source_release()`, the
//! active-child add and remove, `obs_source_get_name()` and `blog()`).
//!
//! `libobs/obs-source-audio-settings.c.h`, the source audio settings, have
//! their shims in [`ffi::source_audio_settings`], over
//! `libobs/obs-source-audio-settings-seam.h`.
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
