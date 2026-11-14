//! The clock drift error of audio on a clock of its own (#4182).
//!
//! Audio from a sender with its own clock (NDI through obs-avip or DistroAV,
//! #3747) is placed by its own timestamps, and a packet that continues the
//! previous one is appended to the source's buffer. When the sender's clock
//! runs faster than the system clock, the buffer gets longer and the audio
//! plays later and later. #4143 bounds that with a step. This module is the
//! error that lets the resampler's clock drift compensation
//! (`obs_media_io::audio_resampler_drift`) take the drift out smoothly
//! instead.
//!
//! The error is the end of the buffered audio against the packet's arrival:
//! [`buffered_end_gap`] when each packet is appended. That is the quantity
//! that resampling changes. The packet's own timestamp is in the sender's
//! time, which no resampling changes, so a gap measured from it would only
//! drive the loop into its clamp.
//!
//! [`OwnClockDrift`] filters the gap:
//! - a minimum over a sliding window of [`WINDOW_NS`] ([`BUCKETS`] buckets of
//!   [`BUCKET_NS`]), so packet-size and network jitter, which only make a
//!   packet late, do not move it;
//! - less a baseline: the minimum over the first window, the start offset
//!   that is not drift.
//!
//! A positive error means the buffer holds more audio than at the start (a
//! fast sender, too many samples), the sign
//! `audio_resampler_set_compensation_error()` expects. A slow sender gives a
//! negative error.
//!
//! No allocation, no lock, no clock: the caller passes the times.

/// The width of one bucket of the sliding minimum, in nanoseconds.
pub const BUCKET_NS: u64 = 125_000_000;

/// The number of buckets of the sliding minimum.
pub const BUCKETS: usize = 8;

/// The width of the sliding minimum, and of the start window that sets the
/// baseline, in nanoseconds: 1 s.
#[allow(clippy::as_conversions)] // 8 always fits in u64
pub const WINDOW_NS: u64 = BUCKET_NS * BUCKETS as u64;

/// How far the end of the buffered audio is ahead of `os_time`, in
/// nanoseconds: `audio_ts` (the system time of the buffer's first frame)
/// plus `buffered_frames` at `rate`, less `os_time`. Negative when the
/// buffer ends before `os_time`. Saturates at the `i64` range; a `rate` of
/// 0 counts the frames as no time.
pub fn buffered_end_gap(audio_ts: u64, buffered_frames: u64, rate: u32, os_time: u64) -> i64 {
    let frames_ns = if rate == 0 {
        0
    } else {
        i128::from(buffered_frames) * 1_000_000_000 / i128::from(rate)
    };
    let gap = i128::from(audio_ts) + frames_ns - i128::from(os_time);
    i64::try_from(gap).unwrap_or(if gap < 0 { i64::MIN } else { i64::MAX })
}

/// [`BUCKETS`] as `u64`.
#[allow(clippy::as_conversions)] // 8 always fits in u64
const BUCKETS_U64: u64 = BUCKETS as u64;

/// One bucket of the sliding minimum: its number (`os_time / BUCKET_NS`)
/// and the smallest gap in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Bucket {
    id: u64,
    min: i64,
}

/// The filter state of one source: the sliding minimum and the baseline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwnClockDrift {
    /// The arrival of the first sample.
    start: Option<u64>,
    /// The smallest gap in the start window so far.
    start_min: i64,
    /// The start window's minimum, once the window has passed.
    baseline: Option<i64>,
    /// The buckets, each in the slot of its number modulo [`BUCKETS`].
    buckets: [Option<Bucket>; BUCKETS],
}

impl Default for OwnClockDrift {
    fn default() -> Self {
        Self::new()
    }
}

impl OwnClockDrift {
    /// A filter with no samples.
    pub const fn new() -> Self {
        Self {
            start: None,
            start_min: i64::MAX,
            baseline: None,
            buckets: [None; BUCKETS],
        }
    }

    /// Adds the gap of a packet that arrived at `os_time`, and returns the
    /// error for the resampler: the sliding minimum less the baseline.
    /// `None` until a full [`WINDOW_NS`] has passed since the first sample
    /// (the baseline is not known yet), and for a sample older than the
    /// window.
    pub fn sample(&mut self, os_time: u64, gap: i64) -> Option<i64> {
        let id = os_time / BUCKET_NS;
        let newest = self.buckets.iter().flatten().map(|b| b.id).max();
        if newest.is_some_and(|newest| id.saturating_add(BUCKETS_U64) <= newest) {
            return None;
        }

        let start = *self.start.get_or_insert(os_time);
        if self.baseline.is_none() {
            if os_time.saturating_sub(start) < WINDOW_NS {
                self.start_min = self.start_min.min(gap);
                self.insert(id, gap);
                return None;
            }
            self.baseline = Some(self.start_min);
        }

        self.insert(id, gap);
        let current = newest.map_or(id, |newest| newest.max(id));
        let min = self
            .buckets
            .iter()
            .flatten()
            .filter(|b| b.id <= current && current - b.id < BUCKETS_U64)
            .map(|b| b.min)
            .min()
            .unwrap_or(gap);
        self.baseline.map(|baseline| min.saturating_sub(baseline))
    }

    /// Forgets every sample and the baseline: a new stream, or new timing.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Adds `gap` to the bucket of `id`. A bucket of an older window in the
    /// same slot is replaced; a newer one is kept.
    fn insert(&mut self, id: u64, gap: i64) {
        let Some(slot) = self.buckets.get_mut(slot_of(id)) else {
            return;
        };
        *slot = Some(match *slot {
            Some(b) if b.id == id => Bucket {
                id,
                min: b.min.min(gap),
            },
            Some(b) if b.id > id => b,
            _ => Bucket { id, min: gap },
        });
    }
}

/// The slot of bucket `id`: `id` modulo [`BUCKETS`].
#[allow(clippy::as_conversions)] // the remainder is below 8
fn slot_of(id: u64) -> usize {
    (id % BUCKETS_U64) as usize
}
