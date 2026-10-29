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
//! packets to the batch (C divides by 0), and on an MSVC x64 target a
//! quotient above 64 bits saturates (MSVC's `_udiv128` faults, clang-cl
//! truncates it).

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

/// Nanoseconds per second (the `1000000000ULL` of `calculate_batch_size()`).
pub const NS_PER_SEC: u64 = 1_000_000_000;

/// `util_mul_div64()` as the C computes it on this target: `num * mul / div`
/// truncated. On an MSVC x64 target the C uses exact 128-bit math, built as
/// `_umul128`/`_udiv128` by MSVC and as an `unsigned __int128` division by
/// clang-cl; elsewhere `(num / div) * mul + (num % div * mul) / div` in
/// wrapping `u64`. Intentional differences: a `div` of 0 (undefined in C)
/// gives 0, and a quotient above 64 bits on an MSVC x64 target saturates,
/// where MSVC's `_udiv128` faults and clang-cl's `(uint64_t)` cast
/// truncates it.
#[must_use]
pub fn mul_div64(num: u64, mul: u64, div: u64) -> u64 {
    if obs_util::platform::MUL_DIV64_IS_UDIV128 {
        let quotient = u128::from(num)
            .wrapping_mul(u128::from(mul))
            .checked_div(u128::from(div))
            .unwrap_or(0);
        u64::try_from(quotient).unwrap_or(u64::MAX)
    } else {
        let rem = num.checked_rem(div).unwrap_or(0);
        num.checked_div(div)
            .unwrap_or(0)
            .wrapping_mul(mul)
            .wrapping_add(rem.wrapping_mul(mul).checked_div(div).unwrap_or(0))
    }
}

/// `calculate_batch_size()`: a video encoder's packet interval in ns,
/// `util_mul_div64(1000000000, fps_den * frame_rate_divisor, fps_num)`, the
/// product in `uint32_t`.
#[must_use]
pub fn video_interval(fps_num: u32, fps_den: u32, frame_rate_divisor: u32) -> u64 {
    let den = fps_den.wrapping_mul(frame_rate_divisor);
    mul_div64(NS_PER_SEC, u64::from(den), u64::from(fps_num))
}

/// `calculate_batch_size()`: an audio encoder's packet interval in ns,
/// `util_mul_div64(1000000000, frame_size, sample_rate)`.
#[must_use]
pub fn audio_interval(sample_rate: u32, frame_size: usize) -> u64 {
    // size_t to uint64_t: lossless on every target libobs builds for.
    let frame_size = u64::try_from(frame_size).unwrap_or(u64::MAX);
    mul_div64(NS_PER_SEC, frame_size, u64::from(sample_rate))
}

/// `calculate_batch_size()`: the sum over the intervals of
/// `(largest * 2) / interval` (the doubling wraps as the C's `uint64_t`),
/// `largest` being the largest of them; each term converted to `size_t` and
/// added with wrapping, as the C does. No interval: 0.
#[must_use]
pub fn batch_size(intervals: &[u64]) -> usize {
    let largest = intervals.iter().copied().max().unwrap_or(0);
    let doubled = largest.wrapping_mul(2);
    intervals.iter().fold(0usize, |batch, &interval| {
        // None only for an interval of 0, an intentional difference (module
        // docs): it adds nothing. uint64_t to size_t is lossless on the
        // 64-bit targets libobs builds for.
        let packets = doubled.checked_div(interval).unwrap_or(0);
        batch.wrapping_add(usize::try_from(packets).unwrap_or(usize::MAX))
    })
}

/// `pair_encoders()`: true when the first video encoder takes part, that is
/// when it is not active yet.
#[must_use]
pub fn pair_video(video_active: bool) -> bool {
    !video_active
}

/// `pair_encoders()`: true when this audio encoder is paired with the video
/// encoder: it is not active and has no paired encoder yet.
#[must_use]
pub fn pair_audio(audio_active: bool, paired_num: usize) -> bool {
    !audio_active && paired_num == 0
}
