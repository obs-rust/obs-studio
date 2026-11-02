//! Safe core of `libobs/obs-source-transition-sample-time.c.h`: the time
//! of each audio sample a transition mixes, `get_sample_time()` of
//! `libobs/obs-source-transition.c.h`.
//!
//! `mix_child()` scales every sample of a child's audio block by the
//! transition plugin's `mix(context_data, t)`, where `t` is the
//! transition's position at that sample:
//! - [`offset`]: the sample's offset from the start of the block,
//!   `util_mul_div64(sample, 1000000000, sample_rate)` ns;
//! - [`sample_time`]: the clock ([`transition_clock::time`]) at the
//!   block's timestamp plus that offset, the sum wrapping as the C's
//!   `uint64_t` does.
//!
//! `obs-source-transition.c.h` keeps the four clock field reads (once per
//! sample, inside `mix_child()`'s loop), `calc_time()` for its other
//! callers, the mix and channel loops and the plugin's callback. Nothing
//! here keeps state, takes a lock or calls libobs.
//!
//! One intentional difference from the C: a sample rate of 0 gives every
//! sample an offset of 0, so the whole block takes the block's own time.
//! The C divides by zero there.
//!
//! [`transition_clock::time`]: crate::transition_clock::time

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

use crate::transition_clock::Clock;

/// The offset of sample `sample` from the start of a block of
/// `sample_rate` Hz, in ns: `util_mul_div64(sample, 1000000000,
/// sample_rate)` as the C computes it on this host (see
/// `obs_util::util_uint64::mul_div64`), without its panics.
///
/// A sample rate of 0 gives 0, where the C divides by zero (intentional
/// difference). On MSVC x64 the C divides exactly in 128 bits, and a
/// quotient past 64 bits is cut to its low 64, as clang-cl's `(uint64_t)`
/// cast does; MSVC's `_udiv128` faults there, so no result matches it.
/// `mix_child()` passes a sample index under `AUDIO_OUTPUT_FRAMES`, whose
/// quotient always fits.
#[must_use]
pub fn offset(sample_rate: usize, sample: usize) -> u64 {
    let _ = (sample_rate, sample);
    todo!()
}

/// `obs_source_transition_sample_time()` (`get_sample_time()`): the
/// transition's position at sample `sample` of a block of `sample_rate` Hz
/// that starts at `ts`. `ts` plus the [`offset`] wraps, as the C's
/// `uint64_t` sum does.
#[must_use]
pub fn sample_time(clock: Clock, ts: u64, sample_rate: usize, sample: usize) -> f32 {
    let _ = (clock, ts, sample_rate, sample);
    todo!()
}
