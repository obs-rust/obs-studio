//! Safe core of `libobs/obs-encoder-audio-start-frames.c.h`: the two
//! sample-rate conversions of the encoder's audio start in `obs-encoder.c`.
//!
//! - [`offset`]: `calc_offset_size()`'s offset from the audio's start to the
//!   video's in frames, `util_mul_div64(offset_ns, samplerate, 1000000000)`,
//!   which `obs-encoder.c` passes to [`encoder_audio_start::offset_size`].
//! - [`duration`]: `buffer_audio()`'s packet length in nanoseconds,
//!   `util_mul_div64(frames, 1000000000, samplerate)`, which it passes to
//!   [`encoder_audio_start::packet_start`].
//!
//! `obs-encoder.c` keeps every read of the encoder (`samplerate`,
//! `blocksize`), the paired encoder lookup, the deques and every rule call
//! ([`encoder_audio_start`]), each where it was and under its condition, and
//! reads `samplerate` once for each call here. These keep no state and call
//! nothing in libobs.
//!
//! Both conversions go through [`obs_util::util_uint64::mul_div64`], which
//! is the C's `util_mul_div64()` on each platform. With a `u32` rate and
//! `u32` frames, [`duration`] never overflows on any platform. [`offset`]
//! fits too for any offset at rates up to 10^9 Hz; past that, an offset of
//! centuries keeps the platform's result, as the C does: it wraps where the
//! C wraps and panics where MSVC x64's `_udiv128` faults.
//!
//! Intentional difference: at a sample rate of 0, [`duration`] is 0, where
//! the C divides by zero (SIGFPE on x86, a trap on MSVC x64). The rate comes
//! from the encoder plugin's audio info, unchecked. With a length of 0 the
//! packet ends at its own timestamp, so [`encoder_audio_start::packet_start`]
//! waits when it starts at or before the video and starts from the buffer
//! otherwise; it never truncates. [`offset`] divides only by 10^9 and is
//! already 0 at a rate of 0.

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

use crate::delay::NS_PER_SEC;
#[cfg(doc)]
use crate::encoder_audio_start;
use obs_util::util_uint64::mul_div64;

/// `calc_offset_size()`: the frames in `offset_ns` nanoseconds at
/// `samplerate`, rounded down.
#[must_use]
pub fn offset(offset_ns: u64, samplerate: u32) -> u64 {
    mul_div64(offset_ns, u64::from(samplerate), NS_PER_SEC)
}

/// `buffer_audio()`, once the paired video has started: the packet's length
/// in nanoseconds, `frames` at `samplerate`, rounded down. 0 at a
/// `samplerate` of 0 (the C divides by zero).
#[must_use]
pub fn duration(frames: u32, samplerate: u32) -> u64 {
    if samplerate == 0 {
        return 0;
    }
    mul_div64(u64::from(frames), NS_PER_SEC, u64::from(samplerate))
}
