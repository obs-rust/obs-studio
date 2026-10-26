//! Safe core of `libobs/obs-output-stop.c.h`: the output stop and
//! end-capture decisions of `obs-output.c`.
//!
//! One function per decision, each asked where the C makes it.
//! `obs-output.c` keeps `obs_output_valid()`, the `context.data` and
//! `raw_audio2` tests, every atomic load and store (`active`,
//! `data_active`, `reconnecting`, `starting`, `delay_active`,
//! `delay_capturing`, `delay_restart_refs` and the end-capture thread's
//! flags) and the stopping event test, each loaded where it always was and
//! tested in its `&&` order, the `signal` and `force` tests, the unpause,
//! the event resets and signals, the reconnect thread's join, `info.stop`,
//! `obs_output_end_data_capture()`, `obs_output_delay_stop()`,
//! `obs_encoder_stop()`, `obs_encoder_shutdown_unstarted()`,
//! `stop_raw_video()`, `audio_output_disconnect()`, `get_first_mixer()`,
//! `obs_service_deactivate()`, `obs_output_cleanup_delay()`,
//! `log_frame_info()`, the caption frees under their mutex, the keyframe
//! tracking clear, the joins and `pthread_create()`, the stop parameters
//! copied before a thread starts, the reconnect policy, every signal and
//! every log. It passes the values here. These keep no state and call
//! nothing in libobs.
//!
//! - `obs_output_stop()`: [`encoded`] and [`delay_on`] (the delayed stop);
//! - `end_data_capture_thread()`: [`has_video`] and [`has_audio`],
//!   [`encoded`] (the encoders, or the raw video and audio), [`delay_on`]
//!   (the `process_delay()` callback, and the delay cleanup), then
//!   [`interleave`], re-exported from [`crate::raw_audio`], the test
//!   `hook_data_capture()` asks for the same callback, and [`service`] (the
//!   deactivation);
//! - `stop_raw_audio()`: [`mix_on`], re-exported from [`crate::raw_audio`],
//!   `start_raw_audio()`'s test, for each mix with `raw_audio2`; without
//!   it, `get_first_mixer()` asks [`first_mixer`], re-exported from
//!   [`crate::video_checks`];
//! - `obs_output_end_data_capture_internal()`: [`encoded`] (the unstarted
//!   encoders' shutdown), [`end_delay`], [`has_video`] (the frame log) and
//!   [`thread_failed`] (both threads).
//!
//! No input comes from outside OBS (the flags from the output's plugin, the
//! delay and the mixers from the frontend), and nothing here can panic on
//! any input.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

pub use crate::raw_audio::{interleave, mix_on};
pub use crate::video_checks::first_mixer;

/// `OBS_OUTPUT_VIDEO`.
pub const OBS_OUTPUT_VIDEO: u32 = crate::video_checks::OBS_OUTPUT_VIDEO;
/// `OBS_OUTPUT_AUDIO`.
pub const OBS_OUTPUT_AUDIO: u32 = crate::video_checks::OBS_OUTPUT_AUDIO;
/// `OBS_OUTPUT_ENCODED`.
pub const OBS_OUTPUT_ENCODED: u32 = crate::video_checks::OBS_OUTPUT_ENCODED;
/// `OBS_OUTPUT_SERVICE`.
pub const OBS_OUTPUT_SERVICE: u32 = crate::video_checks::OBS_OUTPUT_SERVICE;

/// `obs_output_stop()`, for an active output that is not reconnecting, and
/// `end_data_capture_thread()`: with [`OBS_OUTPUT_ENCODED`] in `flags`
/// (`info.flags`), the stop goes through the delay when one runs, and the
/// encoders are stopped rather than the raw video and audio.
/// `obs_output_end_data_capture_internal()`, for an output that is not
/// active: the encoders it initialized are shut down on a thread.
#[must_use]
pub fn encoded(flags: u32) -> bool {
    (flags & OBS_OUTPUT_ENCODED) != 0
}

/// `obs_output_stop()`, for an encoded output, and
/// `end_data_capture_thread()`: `active_delay_ns` is not 0, so a delay is
/// running: the stop goes through `obs_output_delay_stop()`, the encoders
/// are stopped with `process_delay()` and the delay is cleaned up.
#[must_use]
pub fn delay_on(active_delay_ns: u64) -> bool {
    active_delay_ns != 0
}

/// `end_data_capture_thread()`: with [`OBS_OUTPUT_VIDEO`] in `flags`, the
/// video encoders (or the raw video) are stopped.
/// `obs_output_end_data_capture_internal()`: the frame counts are logged.
#[must_use]
pub fn has_video(flags: u32) -> bool {
    (flags & OBS_OUTPUT_VIDEO) != 0
}

/// `end_data_capture_thread()`: with [`OBS_OUTPUT_AUDIO`] in `flags`, the
/// audio encoders (or the raw audio) are stopped.
#[must_use]
pub fn has_audio(flags: u32) -> bool {
    (flags & OBS_OUTPUT_AUDIO) != 0
}

/// `end_data_capture_thread()`, after the encoders: with
/// [`OBS_OUTPUT_SERVICE`] in `flags`, the service is deactivated.
#[must_use]
pub fn service(flags: u32) -> bool {
    (flags & OBS_OUTPUT_SERVICE) != 0
}

/// `obs_output_end_data_capture_internal()`, for an active output with its
/// data active and a delay active, after `delay_capturing` is cleared:
/// `delay_restart_refs` (loaded by the caller) is 0, so the delay ends with
/// the capture; otherwise a restart keeps it, and the caller sets the
/// stopping event and returns. A negative count keeps it too (the C tests
/// "0").
#[must_use]
pub fn end_delay(delay_restart_refs: i64) -> bool {
    delay_restart_refs == 0
}

/// `obs_output_end_data_capture_internal()`: `pthread_create()` returned
/// `ret`, not 0, so the thread did not start: the unstarted encoders'
/// shutdown is dropped, or `end_data_capture_thread()` runs inline after a
/// warning.
#[must_use]
pub fn thread_failed(ret: i32) -> bool {
    ret != 0
}
