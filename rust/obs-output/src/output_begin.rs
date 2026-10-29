//! Safe core of `libobs/obs-output-begin.c.h`: the begin-capture plan of
//! `obs-output.c`.
//!
//! `obs_output_begin_data_capture_internal()` runs it on every start of an
//! encoded output (stream, recording, replay buffer). `obs-output.c` keeps
//! `obs_get_video_info()`, the encoder getters (frame rate divisor, sample
//! rate, frame size), the walk over the encoder slots and the array of
//! intervals, `get_first_video_encoder_index()`, the `init_mutex` locks, the
//! weak references and the `paired_encoders` pushes, the store to
//! `interleaver_max_batch_size` and the log. It passes their values here.
//! These keep no state, take no lock and call nothing.
//!
//! - [`video_interval`] and [`audio_interval`] (`calculate_batch_size()`,
//!   step 1): each encoder's packet interval in ns;
//! - [`batch_size`] (`calculate_batch_size()`, step 2): the interleaver's
//!   maximum batch size, the sum over the encoders of how many of their
//!   packets fit into twice the largest interval;
//! - [`pair_video`] and [`pair_audio`] (`pair_encoders()`): whether the
//!   first video encoder and each audio encoder take part in the pairing.
//!
//! No input comes from outside OBS (the frame rate from the video settings,
//! the divisors from the frontend, the sample rate and frame size from the
//! encoder plugin), but nothing here can panic on any input. The C's
//! `uint32_t` product `fps_den * frame_rate_divisor` wraps, and so does it
//! here; [`mul_div64`] is `util_mul_div64()` as the C computes it on the
//! target.
//!
//! Intentional differences, unreachable (undefined in C): an `fps_num` or
//! `sample_rate` of 0 gives an interval of 0, an interval of 0 adds no
//! packets to the batch (C divides by 0), and on MSVC x64 a quotient above
//! 64 bits saturates (`_udiv128` faults).

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

/// Nanoseconds per second (the `1000000000ULL` of `calculate_batch_size()`).
pub const NS_PER_SEC: u64 = 1_000_000_000;

/// `util_mul_div64()` as the C computes it on this target: `num * mul / div`
/// truncated. On MSVC x64 the C uses exact 128-bit math; elsewhere
/// `(num / div) * mul + (num % div * mul) / div` in wrapping `u64`.
/// Intentional differences (undefined in C): a `div` of 0 gives 0, and a
/// quotient above 64 bits on MSVC x64 saturates.
#[must_use]
pub fn mul_div64(num: u64, mul: u64, div: u64) -> u64 {
    let _ = (num, mul, div);
    todo!()
}

/// `calculate_batch_size()`: a video encoder's packet interval in ns,
/// `util_mul_div64(1000000000, fps_den * frame_rate_divisor, fps_num)`, the
/// product in `uint32_t`.
#[must_use]
pub fn video_interval(fps_num: u32, fps_den: u32, frame_rate_divisor: u32) -> u64 {
    let _ = (fps_num, fps_den, frame_rate_divisor);
    todo!()
}

/// `calculate_batch_size()`: an audio encoder's packet interval in ns,
/// `util_mul_div64(1000000000, frame_size, sample_rate)`.
#[must_use]
pub fn audio_interval(sample_rate: u32, frame_size: usize) -> u64 {
    let _ = (sample_rate, frame_size);
    todo!()
}

/// `calculate_batch_size()`: the sum over the intervals of
/// `(largest * 2) / interval` (the doubling wraps as the C's `uint64_t`),
/// `largest` being the largest of them; each term converted to `size_t` and
/// added with wrapping, as the C does. No interval: 0.
#[must_use]
pub fn batch_size(intervals: &[u64]) -> usize {
    let _ = intervals;
    todo!()
}

/// `pair_encoders()`: true when the first video encoder takes part, that is
/// when it is not active yet.
#[must_use]
pub fn pair_video(video_active: bool) -> bool {
    let _ = video_active;
    todo!()
}

/// `pair_encoders()`: true when this audio encoder is paired with the video
/// encoder: it is not active and has no paired encoder yet.
#[must_use]
pub fn pair_audio(audio_active: bool, paired_num: usize) -> bool {
    let _ = (audio_active, paired_num);
    todo!()
}
