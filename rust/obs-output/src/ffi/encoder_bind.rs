//! C ABI for `libobs/obs-encoder-bind.c.h`, the encoder media binding and
//! shutdown checks of `obs-encoder.c`, declared in
//! `libobs/obs-encoder-bind.h` without `EXPORT`.
//!
//! `obs-encoder.c.h` (still C) checks the encoder (`obs_encoder_valid()`),
//! logs every warning, loads the active flag and tests `initialized`,
//! `media`, `fps_override` and `context.data` where it always did, takes
//! and tries `init_mutex` and `outputs_mutex` (passing the trylock results
//! here), walks the videos to unbind from and the encoder's outputs (passing
//! `bound` and `needed`), calls `video_output_get_info()`,
//! `audio_output_get_sample_rate()`, `obs_encoder_shutdown()` and the frame
//! rate divisor video's free and create, and writes every answer back into
//! the encoder. So `struct obs_encoder` is never mirrored; the timebase
//! comes back by value as [`obs_encoder_bind_timebase`] (Tier 3 checks its
//! layout against the C). These shims take no lock, call nothing in libobs
//! and dereference no pointer. They stay hidden (`rust-hidden.txt`). The
//! video type refusal is the `obs_encoder_audio_start_not_video` shim in
//! [`super::encoder_audio_start`], which the callers ask too.
//!
//! No intentional differences.

use std::ffi::c_int;

use crate::encoder_bind::{self as core, Timebase};

/// `struct obs_encoder_bind_timebase` (`obs-encoder-bind.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_encoder_bind_timebase {
    pub num: u32,
    pub den: u32,
}

impl From<Timebase> for obs_encoder_bind_timebase {
    fn from(t: Timebase) -> Self {
        Self {
            num: t.num,
            den: t.den,
        }
    }
}

/// `bool obs_encoder_bind_not_audio(int type)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_bind_not_audio(encoder_type: c_int) -> bool {
    core::not_audio(encoder_type)
}

/// `struct obs_encoder_bind_timebase obs_encoder_bind_video_timebase(bool
/// has_video, uint32_t fps_num, uint32_t fps_den)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_bind_video_timebase(
    has_video: bool,
    fps_num: u32,
    fps_den: u32,
) -> obs_encoder_bind_timebase {
    core::video_timebase(has_video, fps_num, fps_den).into()
}

/// `struct obs_encoder_bind_timebase obs_encoder_bind_audio_timebase(bool
/// has_audio, uint32_t sample_rate)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_bind_audio_timebase(
    has_audio: bool,
    sample_rate: u32,
) -> obs_encoder_bind_timebase {
    core::audio_timebase(has_audio, sample_rate).into()
}

/// `bool obs_encoder_bind_divisor_override(uint32_t frame_rate_divisor)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_bind_divisor_override(frame_rate_divisor: u32) -> bool {
    core::divisor_override(frame_rate_divisor)
}

/// `bool obs_encoder_bind_locked(int trylock_result)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_bind_locked(trylock_result: c_int) -> bool {
    core::locked(trylock_result)
}

/// `bool obs_encoder_bind_shutdown_unstarted(bool initialized, bool
/// needed)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_bind_shutdown_unstarted(initialized: bool, needed: bool) -> bool {
    core::shutdown_unstarted(initialized, needed)
}
