//! `obs_reset_audio()` over the C `obs_reset_audio2()`, reached through the
//! private adapter of `libobs/obs-reset-audio-fields.c.h` (#5482). What it
//! is handed is the safe core's ([`crate::reset_audio`]).
use super::audio_info_queries::{obs_audio_info, obs_audio_info2};
use crate::audio_info_queries::AudioInfo;

/// `obs_reset_audio2`.
pub type Reset2 = unsafe extern "C" fn(*const obs_audio_info2) -> bool;

unsafe extern "C" {
    fn obs_reset_audio_fields() -> Reset2;
}

/// Resets audio with the rate and layout of `oai`, the default maximum
/// buffering and dynamic buffering.
///
/// # Safety
/// As for the C function: `oai` points to a readable `struct obs_audio_info`
/// (the original reads through it unchecked, so NULL is outside the
/// contract), and the caller observes `obs_reset_audio2()`'s own
/// requirements.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_reset_audio(oai: *const obs_audio_info) -> bool {
    // SAFETY: the caller passes a readable `struct obs_audio_info`.
    let info = unsafe {
        AudioInfo {
            samples_per_sec: (*oai).samples_per_sec,
            speakers: (*oai).speakers,
        }
    };
    // SAFETY: the adapter returns the address of the C `obs_reset_audio2`.
    let reset2 = unsafe { obs_reset_audio_fields() };
    crate::reset_audio::reset_audio(info, |info2| {
        let oai2 = obs_audio_info2 {
            samples_per_sec: info2.samples_per_sec,
            speakers: info2.speakers,
            max_buffering_ms: info2.max_buffering_ms,
            fixed_buffering: info2.fixed_buffering,
        };
        // SAFETY: a live, fully initialized `struct obs_audio_info2` that
        // outlives the call, as the original's stack struct does.
        unsafe { reset2(&oai2) }
    })
}
