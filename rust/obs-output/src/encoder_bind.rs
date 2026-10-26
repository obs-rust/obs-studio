//! Safe core of `libobs/obs-encoder-bind.c.h`: the encoder media binding
//! and shutdown checks of `obs-encoder.c`.
//!
//! One function per decision, each asked where the C makes it.
//! `obs-encoder.c` keeps `obs_encoder_valid()`, every warning, the active
//! flag (an atomic, loaded where it always was) and the `initialized`,
//! `media`, `fps_override` and `context.data` tests (each a single field,
//! tested where it always was, in its `&&` order), `init_mutex` and
//! `outputs_mutex` with both `pthread_mutex_trylock()` calls and their
//! unlock order, the walk over the videos to unbind from (it computes
//! `bound`) and the walk over the encoder's outputs (it computes `needed`),
//! each stopping where it always did, `obs_encoder_shutdown()`,
//! `video_output_get_info()`, `audio_output_get_sample_rate()`, the frame
//! rate divisor video's free and create, and every write to the encoder. It
//! passes their values here. These keep no state and call nothing in
//! libobs.
//!
//! - `obs_encoder_set_audio()` and `obs_encoder_audio()`: [`not_audio`]
//!   (the type refusal). The video setter, `obs_encoder_video()`,
//!   `obs_encoder_parent_video()` and `obs_encoder_unbind_video()` ask
//!   [`not_video`], re-exported from [`crate::encoder_audio_start`], the
//!   refusal `obs_encoder_set_gpu_scale_type()` already asks;
//! - `encoder_set_video()`: [`video_timebase`] and [`divisor_override`];
//! - `obs_encoder_set_audio()`: [`audio_timebase`];
//! - `obs_encoder_shutdown_unstarted()`: [`locked`] (each trylock result)
//!   and [`shutdown_unstarted`].
//!
//! No input comes from outside OBS (the frame rate and sample rate from the
//! video and audio settings, the divisor from the frontend), and nothing
//! here can panic on any input. Nothing here divides: the timebase is two
//! copies.

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

pub use crate::encoder_audio_start::not_video;

/// `OBS_ENCODER_AUDIO` (`enum obs_encoder_type`).
pub const OBS_ENCODER_AUDIO: i32 = crate::encoder_size::OBS_ENCODER_AUDIO;

/// An encoder's `timebase_num` and `timebase_den`
/// (`struct obs_encoder_bind_timebase`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Timebase {
    pub num: u32,
    pub den: u32,
}

/// `obs_encoder_set_audio()` and `obs_encoder_audio()` after
/// `obs_encoder_valid()`: `encoder_type` (`info.type`) is not
/// [`OBS_ENCODER_AUDIO`], so the caller logs and refuses. The setter's
/// active refusal follows in the C; it has no initialized refusal.
#[must_use]
pub fn not_audio(encoder_type: i32) -> bool {
    encoder_type != OBS_ENCODER_AUDIO
}

/// `encoder_set_video()`: with a video, the frame period, `fps_den` over
/// `fps_num`; without one, 0 over 0.
#[must_use]
pub fn video_timebase(has_video: bool, fps_num: u32, fps_den: u32) -> Timebase {
    if has_video {
        Timebase {
            num: fps_den,
            den: fps_num,
        }
    } else {
        Timebase::default()
    }
}

/// `obs_encoder_set_audio()` after the refusals: with an audio, 1 over the
/// sample rate; without one, 0 over 0.
#[must_use]
pub fn audio_timebase(has_audio: bool, sample_rate: u32) -> Timebase {
    if has_audio {
        Timebase {
            num: 1,
            den: sample_rate,
        }
    } else {
        Timebase::default()
    }
}

/// `encoder_set_video()` with a video: the frame rate divisor is not 0, so
/// the video with that divisor (`fps_override`) is created.
#[must_use]
pub fn divisor_override(frame_rate_divisor: u32) -> bool {
    frame_rate_divisor != 0
}

/// `obs_encoder_shutdown_unstarted()`: a `pthread_mutex_trylock()` result
/// of 0, so the mutex is held. Asked for `outputs_mutex`, then for
/// `init_mutex` only when `outputs_mutex` is held.
#[must_use]
pub fn locked(trylock_result: i32) -> bool {
    trylock_result == 0
}

/// `obs_encoder_shutdown_unstarted()` with both mutexes held: shut the
/// encoder down when it is initialized and not needed (active, or an output
/// that needs its encoders; the caller's walk stops at the first).
#[must_use]
pub fn shutdown_unstarted(initialized: bool, needed: bool) -> bool {
    initialized && !needed
}
