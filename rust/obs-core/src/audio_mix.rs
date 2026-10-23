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

/// `obs_audio_mix_time_to_frames()`: `floor(t * sample_rate / 1e9)` through
/// `util_mul_div64`, as the C computes it on each host.
pub fn time_to_frames(sample_rate: usize, t: u64) -> usize {
    let _ = (sample_rate, t);
    todo!()
}

/// The frame of the tick `ts` that audio starting at `audio_ts` goes to,
/// or `None` when it adds nothing: before the window, at or after its end,
/// or on frame [`AUDIO_OUTPUT_FRAMES`] or later.
pub fn mix_start(audio_ts: u64, ts: TsInfo, sample_rate: usize) -> Option<usize> {
    let _ = (audio_ts, ts, sample_rate);
    todo!()
}

/// The add loop of `obs_audio_mix_add()` for one mix and channel: adds
/// `src[i]` to `dst[start + i]` for every frame of `dst` from `start` on.
pub fn mix_into(dst: &mut [f32], src: &[f32], start: usize) {
    let _ = (dst, src, start);
    todo!()
}

/// `obs_audio_buffering_maxed()`: the total equals the maximum.
pub fn buffering_maxed(buffering: &Buffering) -> bool {
    let _ = buffering;
    todo!()
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
    let _ = (buffering, sample_rate, ts, push_front);
    todo!()
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
    let _ = (buffering, sample_rate, ts, min_ts, push_front);
    todo!()
}
