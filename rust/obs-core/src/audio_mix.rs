//! Safe core of `libobs/obs-audio-mix.c.h`: the arithmetic of the audio
//! tick.
//!
//! Every audio tick (`audio_callback()` in `obs-audio.c`, on the audio
//! thread) mixes [`AUDIO_OUTPUT_FRAMES`] frames. Each root source's audio
//! goes into the output mixes from the frame its timestamp falls on
//! ([`mix_start`], [`mix_into`]). When a source is behind the tick, libobs
//! adds buffering ticks: it moves the tick back by whole ticks and queues
//! the windows it skipped ([`buffering_add`]; [`buffering_fixed`] adds all
//! of them up to the maximum at once).
//!
//! This is only the arithmetic, on plain values. The source and core side
//! stays C (`obs-audio.c.h`): it takes the locks, reads the source's
//! `audio_ts` and `audio_output_buf`, passes the buffering fields of
//! `struct obs_core_audio`, and logs the [`Step`]. Nothing here keeps state,
//! takes a lock or calls into libobs; the buffering functions push their
//! windows through a callback.
//!
//! Characterized, not endorsed (`test_audio_buffering_subsample`), and kept
//! as the C has them:
//! - times round down twice: `audio_frames_to_ns` truncates to whole ns and
//!   [`time_to_frames`] truncates to whole frames, so a frame's own time can
//!   map to the frame before it (frame 1 at 44.1 kHz, frame 512 at 48 kHz);
//! - [`buffering_maxed`] is equality only: a total above the maximum is not
//!   at the maximum;
//! - a delay below one sample still counts as buffering: [`buffering_add`]
//!   returns an added step with 0 ticks and 0 ms, which `obs-audio.c` logs.
//!
//! Intentional differences from C, all where C has undefined behaviour and
//! libobs never gets there:
//! - audio after frame [`AUDIO_OUTPUT_FRAMES`] of a window longer than one
//!   tick: the C makes an out-of-range pointer and, in practice, adds
//!   nothing; here it adds nothing ([`mix_start`] returns `None`);
//! - a negative tick count (a total above the maximum): the C loops until
//!   the `int` wraps; here no window is pushed;
//! - a sample rate of 0: the C divides by 0; here the frames and windows are
//!   0 and the ms values 0;
//! - a tick count whose sum with the total overflows `int` (a delay of
//!   years, from a broken timestamp): the C's sum is undefined and, as it
//!   wraps in practice, can skip the cut at the maximum and push billions of
//!   windows. Here the sum is exact, so the cut applies and at most
//!   `max - total` windows are pushed. The ms values (logged only) wrap;
//! - on MSVC x64, a quotient that does not fit in 64 bits (a rate above
//!   1 GHz): the C faults, here it saturates.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

/// `AUDIO_OUTPUT_FRAMES` (`media-io/audio-io.h`): the frames of one tick.
pub const AUDIO_OUTPUT_FRAMES: usize = 1024;

/// `MAX_AUDIO_MIXES` (`media-io/audio-io.h`): the output mixes (tracks).
pub const MAX_AUDIO_MIXES: usize = 6;

/// `MAX_AUDIO_CHANNELS` (`media-io/audio-io.h`).
pub const MAX_AUDIO_CHANNELS: usize = 8;

/// `struct ts_info`: a tick window in ns, `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TsInfo {
    pub start: u64,
    pub end: u64,
}

/// The buffering fields of `struct obs_core_audio`, by value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Buffering {
    /// `buffered_ts`: the start of the tick the waiting ticks go before.
    pub buffered_ts: u64,
    /// `buffering_wait_ticks`: ticks queued and not yet mixed.
    pub wait_ticks: u64,
    /// `total_buffering_ticks`.
    pub total_ticks: i32,
    /// `max_buffering_ticks`.
    pub max_ticks: i32,
}

/// What a buffering function did (`struct obs_audio_buffering_step`): the
/// values `obs-audio.c` logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Step {
    /// False when the total was at the maximum and nothing changed.
    pub added: bool,
    /// [`buffering_add`] only: the ticks were cut at the maximum.
    pub max_reached: bool,
    pub ticks: i32,
    pub ms: usize,
    pub total_ms: usize,
}

/// `util_mul_div64` (`util/util_uint64.h`), `num * mul / div` truncated,
/// as the C computes it on each platform. Off MSVC x64 that is
/// `(num / div) * mul + (num % div * mul) / div` in wrapping `uint64_t`,
/// which differs from the exact result once `num % div * mul` overflows. On
/// MSVC x64 it is exact 128-bit math (`_umul128`, `_udiv128`), which faults
/// when the quotient does not fit in 64 bits; here that saturates. A `div`
/// of 0 (a sample rate of 0, where the C divides by 0) gives 0.
fn mul_div64(num: u64, mul: u64, div: u64) -> u64 {
    if crate::platform::MUL_DIV64_IS_UDIV128 {
        let quotient = (u128::from(num).wrapping_mul(u128::from(mul)))
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

/// Nanoseconds per second, the unit of the timestamps.
const NS: u64 = 1_000_000_000;

/// [`AUDIO_OUTPUT_FRAMES`] as the C's `int` macro takes part in `uint64_t`
/// and `int` arithmetic.
const FRAMES_U64: u64 = AUDIO_OUTPUT_FRAMES as u64;
const FRAMES_I32: i32 = AUDIO_OUTPUT_FRAMES as i32;
/// The C's `AUDIO_OUTPUT_FRAMES - 1`, to round a frame count up to ticks.
const FRAMES_U64_MINUS_1: u64 = 1023;

/// The C converts `size_t` to `uint64_t` without loss on every host libobs
/// builds for.
fn rate_u64(sample_rate: usize) -> u64 {
    u64::try_from(sample_rate).unwrap_or(u64::MAX)
}

/// `audio_frames_to_ns` (`media-io/audio-io.h`).
fn frames_to_ns(sample_rate: usize, frames: u64) -> u64 {
    mul_div64(frames, NS, rate_u64(sample_rate))
}

/// `ns_to_audio_frames` (`media-io/audio-io.h`).
fn ns_to_frames(sample_rate: usize, ns: u64) -> u64 {
    mul_div64(ns, rate_u64(sample_rate), NS)
}

/// `ticks * AUDIO_OUTPUT_FRAMES * 1000 / sample_rate`: `int` arithmetic,
/// then the C's conversion of the `int` to `size_t` for the division.
fn ticks_to_ms(ticks: i32, sample_rate: usize) -> usize {
    let product = ticks.wrapping_mul(FRAMES_I32).wrapping_mul(1000);
    (product as isize as usize)
        .checked_div(sample_rate)
        .unwrap_or(0)
}

/// `obs_audio_mix_time_to_frames()`: `floor(t * sample_rate / 1e9)` through
/// `util_mul_div64`, as the C computes it on each host.
pub fn time_to_frames(sample_rate: usize, t: u64) -> usize {
    // (size_t) of the uint64_t result: truncates on a 32-bit size_t.
    ns_to_frames(sample_rate, t) as usize
}

/// The frame of the tick `ts` that audio starting at `audio_ts` goes to,
/// or `None` when it adds nothing: before the window, at or after its end,
/// or on frame [`AUDIO_OUTPUT_FRAMES`] or later.
pub fn mix_start(audio_ts: u64, ts: TsInfo, sample_rate: usize) -> Option<usize> {
    if audio_ts < ts.start || ts.end <= audio_ts {
        return None;
    }
    if audio_ts == ts.start {
        return Some(0);
    }
    let start = time_to_frames(sample_rate, audio_ts.wrapping_sub(ts.start));
    (start < AUDIO_OUTPUT_FRAMES).then_some(start)
}

/// The add loop of `obs_audio_mix_add()` for one mix and channel: adds
/// `src[i]` to `dst[start + i]` for every frame of `dst` from `start` on.
pub fn mix_into(dst: &mut [f32], src: &[f32], start: usize) {
    let Some(dst) = dst.get_mut(start..) else {
        return;
    };
    for (mix, aud) in dst.iter_mut().zip(src) {
        *mix += *aud;
    }
}

/// `obs_audio_buffering_maxed()`: the total equals the maximum.
pub fn buffering_maxed(buffering: &Buffering) -> bool {
    buffering.total_ticks == buffering.max_ticks
}

/// The start of the waiting ticks, then one window per added tick pushed to
/// the front, each one tick before the last; `ts` becomes the earliest.
/// With no tick (or a negative count), `ts.start` still becomes the start
/// of the waiting ticks and `ts.end` stays.
fn push_ticks(
    buffering: &mut Buffering,
    sample_rate: usize,
    ts: &mut TsInfo,
    ticks: i32,
    mut push_front: impl FnMut(TsInfo),
) {
    let mut new_ts = *ts;
    new_ts.start = buffering_ts_minus(buffering.buffered_ts, sample_rate, buffering.wait_ticks);
    for _ in 0..ticks.max(0) {
        buffering.wait_ticks = buffering.wait_ticks.wrapping_add(1);
        new_ts.end = new_ts.start;
        new_ts.start = buffering_ts_minus(buffering.buffered_ts, sample_rate, buffering.wait_ticks);
        push_front(new_ts);
    }
    *ts = new_ts;
}

/// `buffered_ts - audio_frames_to_ns(sample_rate, ticks * AUDIO_OUTPUT_FRAMES)`
/// in wrapping `uint64_t`.
fn buffering_ts_minus(buffered_ts: u64, sample_rate: usize, ticks: u64) -> u64 {
    buffered_ts.wrapping_sub(frames_to_ns(sample_rate, ticks.wrapping_mul(FRAMES_U64)))
}

/// `obs_audio_buffering_fixed()`: adds the ticks up to the maximum, pushes
/// one window per tick to the front of the queue (`push_front`) and moves
/// `ts` to the earliest.
pub fn buffering_fixed(
    buffering: &mut Buffering,
    sample_rate: usize,
    ts: &mut TsInfo,
    push_front: impl FnMut(TsInfo),
) -> Step {
    if buffering_maxed(buffering) {
        return Step::default();
    }
    if buffering.wait_ticks == 0 {
        buffering.buffered_ts = ts.start;
    }
    let ticks = buffering.max_ticks.wrapping_sub(buffering.total_ticks);
    buffering.total_ticks = buffering.max_ticks;
    let step = Step {
        added: true,
        max_reached: false,
        ticks,
        ms: ticks_to_ms(ticks, sample_rate),
        total_ms: ticks_to_ms(buffering.total_ticks, sample_rate),
    };
    push_ticks(buffering, sample_rate, ts, ticks, push_front);
    step
}

/// `obs_audio_buffering_add()`: adds the ticks a source at `min_ts`
/// needs, rounded up and cut at the maximum, pushes one window per tick
/// to the front of the queue (`push_front`) and moves `ts` to the
/// earliest.
pub fn buffering_add(
    buffering: &mut Buffering,
    sample_rate: usize,
    ts: &mut TsInfo,
    min_ts: u64,
    push_front: impl FnMut(TsInfo),
) -> Step {
    if buffering_maxed(buffering) {
        return Step::default();
    }
    if buffering.wait_ticks == 0 {
        buffering.buffered_ts = ts.start;
    }
    let frames = ns_to_frames(sample_rate, ts.start.wrapping_sub(min_ts));
    // (int) of the uint64_t tick count: truncates, as GCC, Clang and MSVC do.
    let mut ticks = (frames
        .wrapping_add(FRAMES_U64_MINUS_1)
        .wrapping_div(FRAMES_U64)) as i32;
    let mut max_reached = false;
    let total = i64::from(buffering.total_ticks).wrapping_add(i64::from(ticks));
    let max = i64::from(buffering.max_ticks);
    if total >= max {
        ticks = (i64::from(ticks).wrapping_sub(total.wrapping_sub(max))) as i32;
        buffering.total_ticks = buffering.max_ticks;
        max_reached = true;
    } else {
        buffering.total_ticks = total as i32;
    }
    let step = Step {
        added: true,
        max_reached,
        ticks,
        ms: ticks_to_ms(ticks, sample_rate),
        total_ms: ticks_to_ms(buffering.total_ticks, sample_rate),
    };
    push_ticks(buffering, sample_rate, ts, ticks, push_front);
    step
}
