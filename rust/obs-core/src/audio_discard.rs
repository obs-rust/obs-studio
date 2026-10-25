//! Safe core of `libobs/obs-audio-discard.c.h`: the steps of the audio tick
//! that drop audio from a source's input buffer.
//!
//! Every audio tick (`audio_callback()` in `obs-audio.c`, on the audio
//! thread) takes [`AUDIO_OUTPUT_FRAMES`] frames of each source's input.
//! [`ignore`] drops the audio of a source that lags behind the tick at
//! maximum buffering, or restarts the source's audio when that is not
//! enough. After the mix, [`discard_tick`] drops the frames of the tick and
//! moves the source's timestamp to the tick end. [`if_stopped`] is the
//! "pending stop" check both use: a buffer that keeps its size for two
//! checks belongs to a source that has stopped, so it is cleared.
//!
//! This is only the decision, on the source's fields by value
//! ([`InputState`]) and the byte size of its channel 0 buffer. The C side
//! (`obs-audio.c.h`, and the shim in [`crate::ffi::audio_discard`]) holds
//! the source's `audio_buf_mutex`, applies the returned [`Pop`] to the
//! input deques and logs the step. Nothing here keeps state, takes a lock
//! or calls into libobs.
//!
//! Characterized, not endorsed (`test_audio_buffering_subsample`), and kept
//! as the C has them:
//! - the sample count is channel 0's, and every channel pops it
//!   ([`Pop::Bytes`]); only the stop check clears each channel by its own
//!   size ([`Pop::EachAll`]);
//! - [`ignore`] without a timestamp pops channel 0's size from each
//!   channel, read again for each one, so only channel 0 is cleared and the
//!   other channels keep their audio ([`Pop::Channel0`]);
//! - the drop is rounded up, so a source 1 ns behind drops a whole sample
//!   and lands after the tick start; the lag [`ignore`] reports for the
//!   log is then `start_ts - audio_ts` in wrapping `u64`.
//!
//! All arithmetic is the C's `uint64_t` and `size_t` arithmetic, which
//! wraps where the C wraps.
//!
//! Intentional difference from C, where C has undefined behaviour and
//! libobs never gets there: with a sample rate of 0, the time of the
//! dropped samples is 0 (the C divides by 0), so [`ignore`] drops one
//! sample and restarts the source.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

use crate::audio_mix::{AUDIO_OUTPUT_FRAMES, NS, TsInfo, mul_div64, rate_u64};

/// `sizeof(float)`: the deques hold `f32` samples.
const SAMPLE_SIZE: usize = 4;

/// `AUDIO_OUTPUT_FRAMES * sizeof(float)`: one tick of one channel in bytes.
const MAX_AUDIO_SIZE: usize = AUDIO_OUTPUT_FRAMES * SAMPLE_SIZE;

/// The fields of `struct obs_source` the steps read and change, under the
/// source's `audio_buf_mutex`. `timing_set` is not here: the steps only
/// clear it, which [`IgnoreStep::restarted`] reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InputState {
    /// `audio_ts`: the time of the first buffered sample, 0 for none.
    pub audio_ts: u64,
    /// `last_audio_input_buf_size`: channel 0's size at the last check.
    pub last_size: usize,
    /// `audio_pending`.
    pub pending: bool,
    /// `pending_stop`.
    pub pending_stop: bool,
}

/// What the caller pops from the front of the input deques.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pop {
    /// Nothing.
    #[default]
    None,
    /// This many bytes from each of the first `channels` deques.
    Bytes(usize),
    /// This many bytes from channel 0 only: the C pops channel 0's size,
    /// read again for each channel, so the others pop 0 once channel 0 is
    /// empty.
    Channel0(usize),
    /// Each of the first `channels` deques by its own size (all of it).
    EachAll,
}

/// `struct obs_audio_ignore_step`: what [`ignore`] did, for the caller's
/// log lines and its decision to render the source again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IgnoreStep {
    /// The source is back in sync: render it again.
    pub rerender: bool,
    /// Audio without a timestamp: channel 0 was cleared.
    pub no_timestamp: bool,
    /// Channel 0's sample count before the step.
    pub buffered: usize,
    /// The samples popped from each channel (0 when none were).
    pub dropped: usize,
    /// With dropped samples: `start_ts - audio_ts` after the drop, before
    /// the 1 ns adjustment (wrapping).
    pub lag: u64,
    /// The source's audio restarted and the caller prints the warning.
    pub warn: bool,
    /// `start_ts - audio_ts` before the restart, for the warning.
    pub warn_lag: u64,
    /// The source's audio restarted: the caller clears `timing_set`.
    pub restarted: bool,
}

/// `enum obs_audio_stop_check`: what [`if_stopped`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StopCheck {
    /// Nothing, or the new size was remembered.
    #[default]
    None,
    /// First unchanged check: `pending_stop` is now set.
    Pending,
    /// Second unchanged check: the buffers are cleared.
    Cleared,
}

/// `enum obs_audio_discard_path`: the path [`discard_tick`] took.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiscardPath {
    /// A source with `audio_render`: `audio_ts` reset, nothing popped.
    RenderSource,
    /// `audio_ts` at or after the tick end.
    Ahead,
    /// `audio_ts` before `ts.start - 1`.
    Behind,
    /// The start point is [`AUDIO_OUTPUT_FRAMES`].
    StartAtEnd,
    /// Not enough audio for the tick: `audio_ts` moved to the tick end,
    /// unless the stop check cleared the buffers.
    Pending,
    /// The tick's frames were popped and `audio_ts` moved to the tick end.
    #[default]
    Done,
}

/// `struct obs_audio_discard_step`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DiscardStep {
    pub path: DiscardPath,
    pub stop: StopCheck,
}

/// `ignore_audio`: a source behind `start_ts` at maximum buffering, with
/// `buffered_bytes` in channel 0. `state.audio_ts` is below `start_ts` (the
/// caller checks it).
pub fn ignore(
    state: &mut InputState,
    buffered_bytes: usize,
    sample_rate: usize,
    start_ts: u64,
) -> (IgnoreStep, Pop) {
    let num_floats = buffered_bytes / SAMPLE_SIZE;
    let mut step = IgnoreStep {
        buffered: num_floats,
        ..IgnoreStep::default()
    };
    let mut pop = Pop::None;

    if state.audio_ts == 0 && num_floats != 0 {
        step.no_timestamp = true;
        state.last_size = 0;
        return (step, Pop::Channel0(buffered_bytes));
    }

    if num_floats != 0 {
        // Round the samples to drop up: (size_t)(lag - 1 in samples) + 1.
        let behind = start_ts.wrapping_sub(state.audio_ts).wrapping_sub(1);
        let drop = (mul_div64(behind, rate_u64(sample_rate), NS) as usize)
            .wrapping_add(1)
            .min(num_floats);

        step.dropped = drop;
        pop = Pop::Bytes(drop.wrapping_mul(SAMPLE_SIZE));

        state.last_size = 0;
        state.audio_ts =
            state
                .audio_ts
                .wrapping_add(mul_div64(drop as u64, NS, rate_u64(sample_rate)));
        step.lag = start_ts.wrapping_sub(state.audio_ts);

        // Rounding error, adjust.
        if state.audio_ts == start_ts.wrapping_sub(1) {
            state.audio_ts = start_ts;
        }

        // Back in sync.
        if state.audio_ts >= start_ts {
            step.rerender = true;
            return (step, pop);
        }
    }

    if !state.pending || num_floats != 0 {
        step.warn = true;
        step.warn_lag = start_ts.wrapping_sub(state.audio_ts);
    }

    state.pending = true;
    state.audio_ts = 0;
    step.restarted = true;
    (step, pop)
}

/// `discard_if_stopped`: the pending stop check, on channel 0's `size` in
/// bytes.
pub fn if_stopped(state: &mut InputState, size: usize) -> (StopCheck, Pop) {
    if size == 0 {
        return (StopCheck::None, Pop::None);
    }

    if state.last_size != size {
        state.last_size = size;
        return (StopCheck::None, Pop::None);
    }

    if !state.pending_stop {
        state.pending_stop = true;
        return (StopCheck::Pending, Pop::None);
    }

    state.pending_stop = false;
    state.audio_ts = 0;
    state.last_size = 0;
    (StopCheck::Cleared, Pop::EachAll)
}

/// `discard_audio`: after the mix, drop the frames of the tick `ts` from
/// the input (channel 0 holds `size` bytes) and move `audio_ts` to the
/// tick end.
pub fn discard_tick(
    state: &mut InputState,
    audio_render: bool,
    size: usize,
    sample_rate: usize,
    ts: TsInfo,
) -> (DiscardStep, Pop) {
    let mut step = DiscardStep::default();

    if audio_render {
        state.audio_ts = 0;
        step.path = DiscardPath::RenderSource;
        return (step, Pop::None);
    }

    if ts.end <= state.audio_ts {
        step.path = DiscardPath::Ahead;
        return (step, Pop::None);
    }

    let start_minus_1 = ts.start.wrapping_sub(1);
    if state.audio_ts < start_minus_1 {
        step.path = DiscardPath::Behind;
        let mut pop = Pop::None;
        if state.pending && size < MAX_AUDIO_SIZE {
            (step.stop, pop) = if_stopped(state, size);
        }
        return (step, pop);
    }

    let mut total_floats = AUDIO_OUTPUT_FRAMES;
    if state.audio_ts != ts.start && state.audio_ts != start_minus_1 {
        let since_start = state.audio_ts.wrapping_sub(ts.start);
        let start_point = mul_div64(since_start, rate_u64(sample_rate), NS) as usize;
        if start_point == AUDIO_OUTPUT_FRAMES {
            step.path = DiscardPath::StartAtEnd;
            return (step, Pop::None);
        }
        total_floats = total_floats.wrapping_sub(start_point);
    }

    let tick_size = total_floats.wrapping_mul(SAMPLE_SIZE);

    if size < tick_size {
        step.path = DiscardPath::Pending;
        let pop;
        (step.stop, pop) = if_stopped(state, size);
        if step.stop != StopCheck::Cleared {
            state.audio_ts = ts.end;
        }
        return (step, pop);
    }

    state.last_size = 0;
    state.pending_stop = false;
    state.audio_ts = ts.end;
    (step, Pop::Bytes(tick_size))
}
