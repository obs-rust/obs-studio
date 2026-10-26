//! Safe core of `libobs/obs-output-start.c.h`: the output start and
//! begin-capture decisions of `obs-output.c`.
//!
//! One function per decision, each asked where the C makes it.
//! `obs-output.c` keeps `obs_output_valid()`, `log_flag_encoded()` (it logs
//! as it tests), the `context.data` test, every atomic load and store
//! (`starting`, `delay_active`, `active`, `reconnecting`, `data_active`,
//! `delay_capturing` and `delay_restart_refs`, each loaded where it always
//! was and tested in its `&&` order), the `success` and
//! `audio_conversion_set` tests, the stopping event wait, `info.start`,
//! `obs_video_stats_frame_counts()`, the caption track reset under its
//! mutex, `obs_service_can_try_to_connect()`, `obs_service_initialize()`
//! and `obs_service_activate()`, `obs_output_delay_start()`, the encoder
//! initialization and the last-error copy, `can_begin_data_capture()`,
//! `pair_encoders()` (its locks and weak references),
//! `hook_data_capture()`, `calculate_batch_size()`, the raw audio buffer
//! clear, `audio_output_get_info()`, `get_audio_planes()`,
//! `get_audio_size()`, `pause_reset()`, every signal and every log. It
//! passes the values here. These keep no state and call nothing in libobs.
//!
//! - `obs_output_start()`: [`service`] (the service gate), then
//!   [`delay_on`], re-exported from [`crate::raw_audio`], the test
//!   `hook_data_capture()` asks (a delay that is not 0: the start goes
//!   through `obs_output_delay_start()`);
//! - `obs_output_actual_start()`: [`drop_restart_ref`];
//! - `obs_output_initialize_encoders()`: [`init_video`] and [`init_audio`];
//! - `reset_raw_output()`: [`audio_defaults`], re-exported from
//!   [`crate::encoder_size`], the fill `get_audio_info()` asks (each
//!   conversion field that is set replaces the mix's);
//! - `obs_output_begin_data_capture_internal()`: [`reset_raw`], [`pair`]
//!   and [`service`] (the activation).
//!
//! No input comes from outside OBS (the flags from the output's plugin, the
//! delay and the conversion from the frontend), and nothing here can panic
//! on any input.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

pub use crate::encoder_size::{AudioFill, audio_defaults};
pub use crate::raw_audio::delay_on;

/// `OBS_OUTPUT_VIDEO`.
pub const OBS_OUTPUT_VIDEO: u32 = crate::video_checks::OBS_OUTPUT_VIDEO;
/// `OBS_OUTPUT_AUDIO`.
pub const OBS_OUTPUT_AUDIO: u32 = crate::video_checks::OBS_OUTPUT_AUDIO;
/// `OBS_OUTPUT_ENCODED`.
pub const OBS_OUTPUT_ENCODED: u32 = crate::video_checks::OBS_OUTPUT_ENCODED;
/// `OBS_OUTPUT_SERVICE`.
pub const OBS_OUTPUT_SERVICE: u32 = crate::video_checks::OBS_OUTPUT_SERVICE;

/// `obs_output_start()`, after the "already starting" refusal: with
/// [`OBS_OUTPUT_SERVICE`] in `flags` (`info.flags`), the start needs the
/// service, which the caller then asks whether it can connect and
/// initializes, in that order. `obs_output_begin_data_capture_internal()`,
/// after the hookup: the service is activated.
#[must_use]
pub fn service(flags: u32) -> bool {
    (flags & OBS_OUTPUT_SERVICE) != 0
}

/// `obs_output_actual_start()`, after `info.start` and the `starting`
/// store: `delay_restart_refs` (loaded by the caller) is not 0, so a
/// delayed restart is under way and the caller drops one reference. A
/// negative count is dropped too (the C tests "not 0").
#[must_use]
pub fn drop_restart_ref(delay_restart_refs: i64) -> bool {
    delay_restart_refs != 0
}

/// `obs_output_initialize_encoders()`, for an encoded output that is not
/// active: with [`OBS_OUTPUT_VIDEO`] in `flags`, the video encoders are
/// initialized (a failure returns before the audio ones).
#[must_use]
pub fn init_video(flags: u32) -> bool {
    (flags & OBS_OUTPUT_VIDEO) != 0
}

/// `obs_output_initialize_encoders()`, after the video encoders: with
/// [`OBS_OUTPUT_AUDIO`] in `flags`, the audio encoders are initialized.
#[must_use]
pub fn init_audio(flags: u32) -> bool {
    (flags & OBS_OUTPUT_AUDIO) != 0
}

/// `obs_output_begin_data_capture_internal()`, for an output that is
/// neither delayed nor active: without [`OBS_OUTPUT_ENCODED`] in `flags` (a
/// raw output), `reset_raw_output()` runs before the capture check.
#[must_use]
pub fn reset_raw(flags: u32) -> bool {
    (flags & OBS_OUTPUT_ENCODED) == 0
}

/// `obs_output_begin_data_capture_internal()`, after the capture check:
/// with both [`OBS_OUTPUT_VIDEO`] and [`OBS_OUTPUT_AUDIO`] in `flags`, the
/// video and audio encoders are paired.
#[must_use]
pub fn pair(flags: u32) -> bool {
    (flags & OBS_OUTPUT_VIDEO) != 0 && (flags & OBS_OUTPUT_AUDIO) != 0
}
