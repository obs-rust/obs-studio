//! Safe core of `libobs/audio-monitoring/monitor-delay.c.h`: the delay
//! decisions of the WASAPI monitor's `process_audio_delay()`
//! (`win32/wasapi-output.c`).
//!
//! A monitored source with video has its audio queued, and each packet at
//! the front of the queue is held back, cut or played to stay in step with
//! the source's video:
//!
//! - [`stale`]: the stale cut. More than [`STALE_NS`] since the last packet
//!   arrived empties the queue; the receive time is stored either way.
//! - the front packet's [`Verdict`], in the steps `process_audio_delay()`
//!   takes: [`diff`] (its timestamp less the device padding, and the
//!   difference from the source's last video timestamp), the "bad diff" gate
//!   in two parts, [`unsynced`] (no video timestamp yet, or more than 5 s
//!   apart) and [`stalled`] (the video stood still), then [`judge`].
//!
//! `wasapi-output.c` keeps the queue (every deque call), `os_gettime_ns()`,
//! the reads of the source and the monitor (the stall count only when
//! [`unsynced`] is false, as before), the `||` between the two parts of the
//! gate, both `util_mul_div64()` calls (the padding arrives here in
//! nanoseconds), the "more are queued" test after the pops, the logs and the
//! loop. The video stall count it passes to [`stalled`] comes from
//! `audio_monitor_track_video_ts()`, which stays a `static inline` in
//! `monitor-video-sync.h`. These keep no state, take no lock and call nothing
//! in WASAPI or libobs.
//!
//! The subtractions wrap and the difference is the two timestamps cast to
//! `i64`, as the C's `uint64_t` arithmetic and casts do. Nothing here can
//! panic on any input, and nothing divides.

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

/// `AUDIO_MONITOR_DELAY_STALE_NS`: more than this since the last packet
/// arrived, the queue is stale.
pub const STALE_NS: u64 = 1_000_000_000;
/// `AUDIO_MONITOR_DELAY_MAX_DIFF_NS`: audio more than this away from the
/// video is not synced to it.
pub const MAX_DIFF_NS: i64 = 5_000_000_000;
/// `AUDIO_MONITOR_DELAY_SYNC_NS`: audio more than this ahead of the video
/// waits; more than this behind it is cut while more is queued.
pub const SYNC_NS: i64 = 75_000_000;
/// `AUDIO_MONITOR_VIDEO_STALL_NS` (`monitor-video-sync.h`): video that has
/// stood still for longer than this is not synced to.
pub const VIDEO_STALL_NS: u64 = 100_000_000;

/// `enum audio_monitor_delay_verdict`: what happens to the front packet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// `AUDIO_MONITOR_DELAY_PLAY`: play it.
    Play,
    /// `AUDIO_MONITOR_DELAY_HOLD`: rushing. Play nothing now and keep the
    /// queue.
    Hold,
    /// `AUDIO_MONITOR_DELAY_DRAG`: dragging. Drop it if more are queued
    /// after it.
    Drag,
}

impl Verdict {
    /// The C enum's value.
    #[must_use]
    pub const fn to_c(self) -> u32 {
        match self {
            Self::Play => 0,
            Self::Hold => 1,
            Self::Drag => 2,
        }
    }
}

/// The front packet against the video: what `audio_monitor_delay_diff()`
/// writes back and returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Front {
    /// The packet's timestamp less the padding (`front_ts`).
    pub front_ts: u64,
    /// Its difference from the video (`diff`).
    pub diff: i64,
}

/// Whether the queue is stale at `cur_time`, the packet's arrival; stores
/// `cur_time` in `last_recv_time`.
pub fn stale(last_recv_time: &mut u64, cur_time: u64) -> bool {
    let stale = cur_time.wrapping_sub(*last_recv_time) > STALE_NS;
    *last_recv_time = cur_time;
    stale
}

/// The packet at the front of the queue, with timestamp `cur_ts`, when the
/// device still has `pad_ns` of audio to play and the source's last video
/// frame is at `last_frame_ts`.
#[must_use]
pub fn diff(cur_ts: u64, pad_ns: u64, last_frame_ts: u64) -> Front {
    let front_ts = cur_ts.wrapping_sub(pad_ns);
    Front {
        front_ts,
        diff: front_ts
            .cast_signed()
            .wrapping_sub(last_frame_ts.cast_signed()),
    }
}

/// The gate's first part: no video frame yet (`last_frame_ts` 0), or audio
/// and video more than [`MAX_DIFF_NS`] apart.
#[must_use]
pub fn unsynced(last_frame_ts: u64, diff: i64) -> bool {
    // llabs() of INT64_MIN is undefined in the C; its magnitude, 2^63, is
    // more than 5 s here.
    last_frame_ts == 0 || diff.unsigned_abs() > MAX_DIFF_NS.unsigned_abs()
}

/// The gate's second part: the video has stood still for more than
/// [`VIDEO_STALL_NS`].
#[must_use]
pub fn stalled(time_since_prev: u64) -> bool {
    time_since_prev > VIDEO_STALL_NS
}

/// The front packet's verdict from the gate (`bad_diff`) and the difference:
/// hold past [`SYNC_NS`] ahead, drag past it behind, else play; play whenever
/// the gate is shut.
#[must_use]
pub fn judge(bad_diff: bool, diff: i64) -> Verdict {
    if !bad_diff && diff > SYNC_NS {
        Verdict::Hold
    } else if !bad_diff && diff < SYNC_NS.wrapping_neg() {
        Verdict::Drag
    } else {
        Verdict::Play
    }
}
