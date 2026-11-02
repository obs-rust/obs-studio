//! Safe core of `libobs/obs-output-raw-audio-trim.c.h`: the two sample-rate
//! conversions of `prepare_audio()` in `obs-output.c`, for a raw-audio
//! packet that starts before the output's first video frame.
//!
//! - [`duration`]: the packet's length in nanoseconds,
//!   `util_mul_div64(frames, 1000000000, sample_rate)`, which `obs-output.c`
//!   passes to [`raw_audio::ends_before`] to drop a packet that ends at or
//!   before the video start.
//! - [`frames`]: the cut in frames, `util_mul_div64(cut_ns, sample_rate,
//!   1000000000)`, for a packet that is kept.
//!
//! `obs-output.c` keeps the passthrough, the video start taken from the
//! pause data under the pause mutex and the wait, the copy of the packet,
//! the before-video test, the drop test (reading the timestamp and the
//! video start after the length), the cut in nanoseconds and the new
//! timestamp, each plane's pointer advance and the new frame count
//! ([`raw_audio`]), each where it was, and reads `sample_rate` once for each
//! call here. These keep no state and call nothing in libobs.
//!
//! Both conversions go through [`obs_util::util_uint64::mul_div64`], which
//! is the C's `util_mul_div64()` on each platform. For a packet's frames (a
//! `u32`) and a cut shorter than the packet, neither quotient overflows on
//! any platform, so neither MSVC x64's fault nor the other platforms'
//! wrapping formula is reached on the cut path. [`frames`] called with a cut
//! longer than any packet keeps the platform's result, as the C does: it
//! wraps where the C wraps and panics where MSVC x64's `_udiv128` faults.
//!
//! Intentional difference: at a sample rate of 0, [`duration`] is 0, where
//! the C divides by zero (SIGFPE on x86). The packet starts before the
//! video, so the caller's drop test then drops it. `obs_reset_audio2()`
//! refuses a zero rate, so libobs does not reach it today. [`frames`]
//! divides only by 10^9 and needs no such case.

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
use crate::raw_audio;
use obs_util::util_uint64::mul_div64;

/// `size_t` to `uint64_t`, as the C's conversion when it passes the rate to
/// `util_mul_div64()`: lossless on every supported target.
fn rate_u64(sample_rate: usize) -> u64 {
    u64::try_from(sample_rate).unwrap_or(u64::MAX)
}

/// `prepare_audio()`, for a packet that starts before the video: its
/// length in nanoseconds, `frames` at `sample_rate`, rounded down. 0 at a
/// `sample_rate` of 0 (the C divides by zero), so the caller drops the
/// packet.
#[must_use]
pub fn duration(frames: u32, sample_rate: usize) -> u64 {
    if sample_rate == 0 {
        return 0;
    }
    mul_div64(u64::from(frames), NS_PER_SEC, rate_u64(sample_rate))
}

/// `prepare_audio()`, for a packet that is kept: the frames in `cut_ns`
/// nanoseconds at `sample_rate`, rounded down.
#[must_use]
pub fn frames(cut_ns: u64, sample_rate: usize) -> u64 {
    mul_div64(cut_ns, rate_u64(sample_rate), NS_PER_SEC)
}
