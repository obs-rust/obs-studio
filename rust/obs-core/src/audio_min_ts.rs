//! Safe core of `libobs/obs-audio-min-ts.c.h`: the per-source decisions of
//! the minimum audio timestamp step of the audio tick.
//!
//! Every audio tick (`audio_callback()` in `obs-audio.c`), libobs looks for
//! the earliest timestamp of the audio sources ([`earlier`]), marks the
//! sources whose input is short for a tick at that timestamp as pending
//! ([`insufficient`]), and then adds buffering when the minimum is before
//! the tick ([`buffering`]).
//!
//! This is only the decision for one source, on plain values. The C side
//! (`obs-audio.c.h`) walks the audio source list, locks each source's
//! `audio_buf_mutex` while it is checked, runs the two passes of
//! `calc_min_ts` and logs. Nothing here keeps state, takes a lock or calls
//! into libobs.
//!
//! Characterized, not endorsed (`test_audio_buffering_subsample`), and kept
//! as the C has them:
//! - [`insufficient`] is false for a start point at or past
//!   [`AUDIO_OUTPUT_FRAMES`] (`>=`), where `discard_audio` stops only on
//!   `==`;
//! - the comparison of [`earlier`] is strict, so ties keep the first source
//!   in the list, and `calc_min_ts`'s second pass, which starts from the
//!   first pass's minimum, never raises it (the C side, characterized
//!   there).
//!
//! All arithmetic is the C's `uint64_t` and `size_t` arithmetic, which
//! wraps where the C wraps; none of it is undefined in C.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

use crate::audio_mix::{AUDIO_OUTPUT_FRAMES, NS, mul_div64, rate_u64};

/// `sizeof(float)`: the input deques hold `f32` samples.
const SAMPLE_SIZE: usize = 4;

/// `enum obs_audio_buffering_choice`: what `audio_callback` does after the
/// search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BufferingChoice {
    /// No buffering.
    #[default]
    None,
    /// `set_fixed_audio_buffering`: fill up to the maximum.
    Fixed,
    /// `add_audio_buffering`: the ticks the minimum is behind.
    Add,
}

/// `find_min_ts`'s test for one source: a source that is not pending, has a
/// timestamp, and is strictly earlier than `min_ts` becomes the minimum.
pub fn earlier(pending: bool, audio_ts: u64, min_ts: &mut u64) -> bool {
    if !pending && audio_ts != 0 && audio_ts < *min_ts {
        *min_ts = audio_ts;
        return true;
    }
    false
}

/// `audio_buffer_insufficient`: whether a source with `buffered` bytes in
/// channel 0 is short for a tick that starts at `min_ts`. A short source is
/// marked pending.
pub fn insufficient(
    audio_render: bool,
    pending: &mut bool,
    audio_ts: u64,
    buffered: usize,
    sample_rate: usize,
    min_ts: u64,
) -> bool {
    if audio_render || *pending || audio_ts == 0 {
        return false;
    }

    let mut total_floats = AUDIO_OUTPUT_FRAMES;
    if audio_ts != min_ts && audio_ts != min_ts.wrapping_sub(1) {
        let since = audio_ts.wrapping_sub(min_ts);
        let start_point = mul_div64(since, rate_u64(sample_rate), NS) as usize;
        if start_point >= AUDIO_OUTPUT_FRAMES {
            return false;
        }
        total_floats = total_floats.wrapping_sub(start_point);
    }

    if buffered < total_floats.wrapping_mul(SAMPLE_SIZE) {
        *pending = true;
        return true;
    }
    false
}

/// The choice after the search: fixed buffering when `fixed_buffer` is set
/// and the buffering is not at the maximum, added buffering when `min_ts`
/// is before the tick start, or none.
pub fn buffering(fixed_buffer: bool, maxed: bool, min_ts: u64, tick_start: u64) -> BufferingChoice {
    if fixed_buffer {
        return if maxed {
            BufferingChoice::None
        } else {
            BufferingChoice::Fixed
        };
    }
    if min_ts < tick_start {
        BufferingChoice::Add
    } else {
        BufferingChoice::None
    }
}
