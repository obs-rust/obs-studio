//! `obs_reset_audio2()` (#5486) over the private adapters of
//! `libobs/obs-reset-audio2-fields.c.h`: the core pointer, its audio output,
//! `obs_free_audio()`, the two buffering fields and `obs_init_audio()` on the
//! "Audio" info stay C. The decisions are the safe core's
//! ([`crate::reset_audio2`]); the two log lines go to `blog()` with the
//! original's format strings, so the text is byte-identical.
use core::ffi::{CStr, c_char, c_int};
use core::marker::PhantomData;

use super::audio_info_queries::obs_audio_info2;
use crate::audio_info_queries::AudioInfo2;
use crate::reset_audio2::{Line, Steps, reset_audio2};

/// `LOG_INFO` (`util/base.h`).
pub const LOG_INFO: c_int = 300;

/// The separator's format.
pub const SEPARATOR_FORMAT: &CStr = c"---------------------------------";

/// The reset message's format, the original's.
pub const RESET_FORMAT: &CStr = c"audio settings reset:\n\tsamples per sec: %d\n\tspeakers:        %d\n\tmax buffering:   %d milliseconds\n\tbuffering type:  %s";

/// `blog()`'s type.
pub type Blog = unsafe extern "C" fn(level: c_int, format: *const c_char, ...);

/// The libobs call the shim makes besides the adapters: `blog()`. The parity
/// test passes the C oracle's recording stand-in, which the oracle's copy of
/// the original logs through too.
pub trait Calls {
    /// `blog`.
    fn blog() -> Blog;
}

/// [`Calls`] through libobs's own `blog()`.
pub enum Libobs {}

unsafe extern "C" {
    fn blog(level: c_int, format: *const c_char, ...);

    fn obs_reset_audio2_core() -> bool;
    fn obs_reset_audio2_has_output() -> bool;
    fn obs_reset_audio2_output_active() -> bool;
    fn obs_reset_audio2_free();
    fn obs_reset_audio2_set_buffering(max_buffering_ticks: c_int, fixed_buffer: bool);
    /// `speakers` is an `enum speaker_layout`, a C `int`.
    fn obs_reset_audio2_init(samples_per_sec: u32, speakers: c_int) -> bool;
}

impl Calls for Libobs {
    fn blog() -> Blog {
        blog
    }
}

/// The adapters, `oai` and `L`'s `blog`.
struct Adapters<L> {
    oai: *const obs_audio_info2,
    calls: PhantomData<L>,
}

impl<L: Calls> Steps for Adapters<L> {
    fn core(&mut self) -> bool {
        // SAFETY: reads the core pointer only.
        unsafe { obs_reset_audio2_core() }
    }
    fn has_output(&mut self) -> bool {
        // SAFETY: the safe core asks only after `core()` held, so `obs` is
        // a live core (the caller's contract).
        unsafe { obs_reset_audio2_has_output() }
    }
    fn output_active(&mut self) -> bool {
        // SAFETY: asked only right after `has_output()` held: a live output.
        unsafe { obs_reset_audio2_output_active() }
    }
    fn free_audio(&mut self) {
        // SAFETY: a live core, past the gates, as the original frees it.
        unsafe { obs_reset_audio2_free() }
    }
    fn info(&mut self) -> Option<AudioInfo2> {
        // SAFETY: `oai` is NULL or points to a readable `struct
        // obs_audio_info2` (the caller's contract); read after the free, as
        // the original reads it.
        let oai = unsafe { self.oai.as_ref() }?;
        Some(AudioInfo2 {
            samples_per_sec: oai.samples_per_sec,
            speakers: oai.speakers,
            max_buffering_ms: oai.max_buffering_ms,
            fixed_buffering: oai.fixed_buffering,
        })
    }
    fn set_buffering(&mut self, max_buffering_ticks: c_int, fixed_buffer: bool) {
        // SAFETY: a live core.
        unsafe { obs_reset_audio2_set_buffering(max_buffering_ticks, fixed_buffer) }
    }
    fn log(&mut self, line: Line) {
        match line {
            // SAFETY: a format with no conversions.
            Line::Separator => unsafe { L::blog()(LOG_INFO, SEPARATOR_FORMAT.as_ptr()) },
            Line::Reset {
                samples_per_sec,
                speakers,
                max_buffering_ms,
                fixed,
            } => {
                let buffering_type: &CStr = if fixed {
                    c"fixed"
                } else {
                    c"dynamically increasing"
                };
                // SAFETY: three `int`s for the three `%d`, then a
                // NUL-terminated string for the `%s`.
                unsafe {
                    L::blog()(
                        LOG_INFO,
                        RESET_FORMAT.as_ptr(),
                        samples_per_sec,
                        speakers,
                        max_buffering_ms,
                        buffering_type.as_ptr(),
                    );
                }
            }
        }
    }
    fn init_audio(&mut self, samples_per_sec: u32, speakers: c_int) -> bool {
        // SAFETY: a live core whose audio was just freed, as the original
        // opens it.
        unsafe { obs_reset_audio2_init(samples_per_sec, speakers) }
    }
}

/// `obs_reset_audio2()` with the `blog()` of `L`.
///
/// # Safety
/// As [`obs_reset_audio2`].
pub unsafe fn obs_reset_audio2_with<L: Calls>(oai: *const obs_audio_info2) -> bool {
    reset_audio2(&mut Adapters::<L> {
        oai,
        calls: PhantomData,
    })
}

/// Resets the core's audio with the settings of `oai`, or only closes it
/// for a NULL `oai`.
///
/// # Safety
/// As for the C function: `oai` is NULL or points to a readable
/// `struct obs_audio_info2`, and the current core, when not NULL, is live
/// and not shutting down on another thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_reset_audio2(oai: *const obs_audio_info2) -> bool {
    // SAFETY: the caller's contract.
    unsafe { obs_reset_audio2_with::<Libobs>(oai) }
}
