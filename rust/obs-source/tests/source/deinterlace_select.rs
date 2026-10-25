//! Tier 1: `obs_source::deinterlace_select`, the deinterlace frame
//! selection of `first_frame()`, `ready_deinterlace_frames()`,
//! `deinterlace_get_closest_frames()` and `deinterlace_update_async_video()`.
//!
//! The `steps_*` cases port `test/cmocka/test_deinterlace_select_steps.c`
//! 1:1, under the same case names, in the same order, with the same inputs
//! and expectations, the stand-in source and the copies of the callers
//! (every erase, release and `prev_frame` write recorded) included.
//!
//! The `edge_*` cases are new: the walk against the C before the move,
//! `ready_deinterlace_frames()` walking a queue of frames and erasing them
//! one by one (written out below), exhaustively over queues of 0 to 6
//! frames on a grid of steps and clocks around the thresholds, and by
//! proptest over any `u64` timestamps (sorted, unsorted, wrapping) and any
//! state; the small decisions over every input.

use obs_source::deinterlace_pacing::{self, TWOX_TOLERANCE};
use obs_source::deinterlace_select::{
    FRAME_TOLERANCE, First, MAX_TS_VAR, OffsetKind, Pair, State, Update, Walk, first,
    take_init_last, take_offset, take_pair, take_reads_flag, unbuffered_pair, update, walk,
};
use proptest::prelude::*;

const MS: u64 = 1_000_000;
const FRAME_TOL: u64 = FRAME_TOLERANCE;
const TWOX: u64 = TWOX_TOLERANCE;
const MAX_VAR: u64 = MAX_TS_VAR;
/// A frame timestamp on the source's clock.
const T: u64 = 1_000_000_000_000;
/// 50 frames a second.
const S: u64 = 20 * MS;
/// The previous tick (`last_sys_timestamp`).
const SYS: u64 = 7_000_000_000_000;
/// `MAX_ASYNC_FRAMES` of `obs-source.c`.
const MAX_ASYNC: usize = 30;

/// Frames at T, T + S, T + 2S, ...
fn regular(n: usize) -> Vec<u64> {
    (0..n).map(|i| T + i as u64 * S).collect()
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Expect {
    drop_front: usize,
    last_frame_ts: u64,
    offset_reset: bool,
    pairs: bool,
    flag_front: bool,
    ready: bool,
}

fn check_walk(r: Walk, e: Expect) {
    assert_eq!(
        r,
        Walk {
            drop_front: e.drop_front,
            last_frame_ts: e.last_frame_ts,
            offset_reset: e.offset_reset,
            pairs: e.pairs,
            flag_front: e.flag_front,
            ready: e.ready,
        }
    );
}

/// A buffered walk whose clock, last + (sys_time - last_sys_timestamp), is
/// `clock`.
fn buffered(ts: &[u64], last: u64, clock: u64, e: Expect) {
    check_walk(
        walk(
            ts,
            SYS.wrapping_add(clock.wrapping_sub(last)),
            State {
                last_frame_ts: last,
                last_sys_timestamp: SYS,
                async_unbuffered: false,
            },
        ),
        e,
    );
}

fn unbuffered(ts: &[u64], e: Expect) {
    check_walk(
        walk(
            ts,
            SYS,
            State {
                last_frame_ts: 12345,
                last_sys_timestamp: SYS - S,
                async_unbuffered: true,
            },
        ),
        e,
    );
}

// ---------------------------------------------------------------------------
// enums

#[test]
fn steps_enum_values() {
    assert_eq!(u32::from(OffsetKind::Buffered), 0);
    assert_eq!(u32::from(OffsetKind::Reanchor), 1);
    assert_eq!(u32::from(Update::None), 0);
    assert_eq!(u32::from(Update::Upload), 1);
    assert_eq!(u32::from(Update::Swap), 2);
    assert_eq!(MAX_VAR, 2_000_000_000);
    assert_eq!(FRAME_TOL, deinterlace_pacing::FRAME_TOLERANCE);
}

// ---------------------------------------------------------------------------
// first

#[test]
fn steps_first_frame_with_one_and_two() {
    for count in 0..5 {
        let r = first(0, count);
        assert!(r.first);
        assert_eq!(r.flag_front, count >= 2);

        // any last_frame_ts, even 1 or one that wrapped, walks
        assert_eq!(
            first(1, count),
            First {
                first: false,
                flag_front: false
            }
        );
        assert_eq!(
            first(u64::MAX, count),
            First {
                first: false,
                flag_front: false
            }
        );
    }
    // characterized, not endorsed: a frame whose timestamp is 0 leaves
    // last_frame_ts at 0, so the next tick takes the first frame again
    assert!(first(0, usize::MAX).flag_front);
}

// ---------------------------------------------------------------------------
// walk: unbuffered

#[test]
fn steps_unbuffered_sizes() {
    let ts = regular(40);

    // nothing queued: nothing to do, last_frame_ts kept
    unbuffered(
        &[],
        Expect {
            last_frame_ts: 12345,
            ..Expect::default()
        },
    );
    unbuffered(
        &ts[..0],
        Expect {
            last_frame_ts: 12345,
            ..Expect::default()
        },
    );
    unbuffered(
        &ts[..1],
        Expect {
            last_frame_ts: T,
            ready: true,
            ..Expect::default()
        },
    );
    unbuffered(
        &ts[..2],
        Expect {
            last_frame_ts: T,
            pairs: true,
            ready: true,
            ..Expect::default()
        },
    );
    for (n, drop) in [(3, 1), (4, 2), (5, 3), (MAX_ASYNC, MAX_ASYNC - 2), (33, 31)] {
        unbuffered(
            &ts[..n],
            Expect {
                drop_front: drop,
                last_frame_ts: T + drop as u64 * S,
                pairs: true,
                ready: true,
                ..Expect::default()
            },
        );
    }
}

#[test]
fn steps_unbuffered_ignores_clock() {
    let ts = [u64::MAX, 0, 5, 3];
    let state = |last_frame_ts, last_sys_timestamp| State {
        last_frame_ts,
        last_sys_timestamp,
        async_unbuffered: true,
    };

    check_walk(
        walk(&ts, 0, state(0, u64::MAX)),
        Expect {
            drop_front: 2,
            last_frame_ts: 5,
            pairs: true,
            ready: true,
            ..Expect::default()
        },
    );
    check_walk(
        walk(&ts[..1], u64::MAX, state(T, 0)),
        Expect {
            last_frame_ts: u64::MAX,
            ready: true,
            ..Expect::default()
        },
    );
}

// ---------------------------------------------------------------------------
// walk: buffered

#[test]
fn steps_buffered_empty() {
    check_walk(
        walk(
            &[],
            SYS,
            State {
                last_frame_ts: T,
                last_sys_timestamp: SYS,
                async_unbuffered: false,
            },
        ),
        Expect {
            last_frame_ts: T,
            ..Expect::default()
        },
    );
}

#[test]
fn steps_buffered_out_of_bounds() {
    let ts = regular(3);
    let reset = |last_frame_ts| Expect {
        last_frame_ts,
        offset_reset: true,
        ready: true,
        ..Expect::default()
    };

    // clock behind the frame
    buffered(&ts, T - MAX_VAR - 1, T - MAX_VAR - 1, reset(T));
    // clock ahead of it
    buffered(&ts, T + MAX_VAR + 1, T + MAX_VAR + 1, reset(T));
    // at MAX_TS_VAR it is in bounds: the clock moves on and nothing is due
    // yet
    buffered(
        &ts,
        T - MAX_VAR,
        T - MAX_VAR + 5 * MS,
        Expect {
            last_frame_ts: T - MAX_VAR + 5 * MS,
            ..Expect::default()
        },
    );
    buffered(
        &ts,
        T - MAX_VAR + 1,
        T - MAX_VAR + 1,
        Expect {
            last_frame_ts: T - MAX_VAR + 1,
            ..Expect::default()
        },
    );
    // ahead by MAX_TS_VAR: in bounds, far past every frame, so it drops to
    // the last two
    buffered(
        &ts,
        T + MAX_VAR,
        T + MAX_VAR,
        Expect {
            drop_front: 1,
            last_frame_ts: T + MAX_VAR,
            offset_reset: true,
            ready: true,
            ..Expect::default()
        },
    );
    // the check uses last_frame_ts before the clock moves
    buffered(&ts[..1], T - MAX_VAR - 1, T, reset(T));
}

#[test]
fn steps_buffered_frame_tolerance() {
    let ts = regular(3);
    let nothing = |last_frame_ts| Expect {
        last_frame_ts,
        ..Expect::default()
    };
    let alone = |last_frame_ts| Expect {
        last_frame_ts,
        ready: true,
        ..Expect::default()
    };

    // before the front frame: nothing taken
    buffered(&ts, T - 10 * MS, T - 5 * MS, nothing(T - 5 * MS));
    buffered(&ts, T - 10 * MS, T, nothing(T));
    // past it by less than the tolerance: still nothing
    buffered(
        &ts,
        T - 10 * MS,
        T + FRAME_TOL - 1,
        nothing(T + FRAME_TOL - 1),
    );
    // at it and past it: the front frame is taken alone
    buffered(&ts, T - 10 * MS, T + FRAME_TOL, alone(T + FRAME_TOL));
    buffered(
        &ts,
        T - 10 * MS,
        T + FRAME_TOL + 1,
        alone(T + FRAME_TOL + 1),
    );
    // the next frame, within the tolerance: the front one alone still
    buffered(&ts, T, T + S + FRAME_TOL - 1, alone(T + S + FRAME_TOL - 1));
    // the next frame due too: the front frame is its pair
    buffered(
        &regular(5),
        T,
        T + S + FRAME_TOL,
        Expect {
            last_frame_ts: T + S + FRAME_TOL,
            flag_front: true,
            ready: true,
            ..Expect::default()
        },
    );
}

#[test]
fn steps_buffered_drops_pairwise() {
    let ts = regular(40);
    let pair = |drop_front, last_frame_ts| Expect {
        drop_front,
        last_frame_ts,
        flag_front: true,
        ready: true,
        ..Expect::default()
    };

    // frames 2 and 3 are due: 0 and 1 are dropped, 2 pairs with 3
    for n in [5, 6, MAX_ASYNC, 33] {
        buffered(&ts[..n], T, T + 3 * S + 5 * MS, pair(2, T + 3 * S + 5 * MS));
    }
    buffered(
        &ts[..33],
        T,
        T + 29 * S + 5 * MS,
        pair(28, T + 29 * S + 5 * MS),
    );
}

#[test]
fn steps_buffered_two_or_fewer_exit() {
    let ts = regular(40);
    let far = T + 100 * S;
    let exit = |drop_front, last_frame_ts| Expect {
        drop_front,
        last_frame_ts,
        offset_reset: true,
        ready: true,
        ..Expect::default()
    };

    // one frame, nothing taken: the exit
    buffered(&ts[..1], T, far, exit(0, far));
    // two frames, nothing taken: the exception takes the first, then the
    // exit with it taken
    buffered(&ts[..2], T, far, exit(0, far));
    // three: the first is dropped behind the second, which is left alone
    // with the third (#2009: no flag)
    buffered(&ts[..3], T, far, exit(1, far));
    buffered(&ts[..4], T, far, exit(2, far));
    buffered(&ts[..5], T, far, exit(3, far));
    buffered(&ts[..MAX_ASYNC], T, far, exit(MAX_ASYNC - 2, far));
    buffered(&ts[..33], T, far, exit(31, far));
    // four frames with the fourth not due: the exit comes first
    buffered(&ts[..4], T, T + 3 * S + 5 * MS, exit(2, T + 3 * S + 5 * MS));
    // two frames with the second within the tolerance: no exit
    buffered(
        &ts[..2],
        T,
        T + S + FRAME_TOL - 1,
        Expect {
            last_frame_ts: T + S + FRAME_TOL - 1,
            ready: true,
            ..Expect::default()
        },
    );
}

#[test]
fn steps_buffered_max_ts_var() {
    for gap in [MAX_VAR - 1, MAX_VAR, MAX_VAR + 1] {
        let ts = [T, T + gap, T + gap + S];
        if gap > MAX_VAR {
            // the clock jumps to the next frame (offset 0 from the front
            // one), which is not due on it
            buffered(
                &ts,
                T,
                T + 5 * MS,
                Expect {
                    last_frame_ts: T + gap,
                    offset_reset: true,
                    ready: true,
                    ..Expect::default()
                },
            );
        } else {
            buffered(
                &ts,
                T,
                T + 5 * MS,
                Expect {
                    last_frame_ts: T + 5 * MS,
                    ready: true,
                    ..Expect::default()
                },
            );
        }
    }

    // the front frame 3 ms behind the clock: the anchor keeps that, so the
    // next frame is due on the new clock too, and the front frame pairs
    // with it across the jump
    let ts = [T, T + MAX_VAR + 1, T + MAX_VAR + 1 + S];
    buffered(
        &ts,
        T + 3 * MS,
        T + 8 * MS,
        Expect {
            last_frame_ts: T + MAX_VAR + 1 + 3 * MS,
            offset_reset: true,
            flag_front: true,
            ready: true,
            ..Expect::default()
        },
    );
}

#[test]
fn steps_buffered_timestamps_backwards() {
    let ts = [T, T - S, T - 2 * S];
    buffered(
        &ts,
        T,
        T + 5 * MS,
        Expect {
            last_frame_ts: T - S,
            offset_reset: true,
            ready: true,
            ..Expect::default()
        },
    );

    // sys_time one ns behind last_sys_timestamp: the clock goes one ns
    // back, nothing is due
    check_walk(
        walk(
            &regular(3),
            SYS - 1,
            State {
                last_frame_ts: T - 10 * MS,
                last_sys_timestamp: SYS,
                async_unbuffered: false,
            },
        ),
        Expect {
            last_frame_ts: T - 10 * MS - 1,
            ..Expect::default()
        },
    );
    // the timestamps wrap past u64::MAX (the step is small, so no
    // re-anchor) and the clock with them, 15 ms short of it: the wrapped
    // frames compare as far behind, so the walk drops to the last two
    let wrap = [u64::MAX - S + 1, 0, S];
    check_walk(
        walk(
            &wrap,
            SYS + 5 * MS,
            State {
                last_frame_ts: u64::MAX - S + 1,
                last_sys_timestamp: SYS,
                async_unbuffered: false,
            },
        ),
        Expect {
            drop_front: 1,
            last_frame_ts: (u64::MAX - S + 1).wrapping_add(5 * MS),
            offset_reset: true,
            ready: true,
            ..Expect::default()
        },
    );
}

// ---------------------------------------------------------------------------
// unbuffered_pair

fn check_pair(r: Pair, prev_frame: bool, frame_ts: Option<u64>) {
    assert_eq!(
        r,
        Pair {
            prev_frame,
            frame_ts
        }
    );
}

#[test]
fn steps_unbuffered_pair_guard() {
    let offset = SYS - T;

    check_pair(unbuffered_pair(T, T + S, 0, SYS), true, None);
    check_pair(unbuffered_pair(T, T + S, 0, 0), true, None);
    check_pair(unbuffered_pair(T, T + S, offset, SYS), false, Some(T - S));
    check_pair(
        unbuffered_pair(T, T + S, offset, SYS + S - 1),
        false,
        Some(T - S),
    );
    check_pair(unbuffered_pair(T, T + S, offset, SYS + S), true, None);
    check_pair(unbuffered_pair(T, T + S, offset, u64::MAX), true, None);
    // characterized, not endorsed: the next frame before the first wraps
    // the duration, so the end wraps to just before timestamp + offset
    check_pair(
        unbuffered_pair(T, T - S, offset, SYS - S - 1),
        false,
        Some(T + S),
    );
    check_pair(unbuffered_pair(T, T - S, offset, SYS - S), true, None);
}

// ---------------------------------------------------------------------------
// take_* and update

#[test]
fn steps_take_decisions() {
    assert!(!take_reads_flag(0));
    assert!(take_reads_flag(1));
    assert!(take_reads_flag(usize::MAX));

    assert!(!take_pair(0, false));
    assert!(!take_pair(0, true));
    assert!(!take_pair(1, false));
    assert!(take_pair(1, true));
    assert!(take_pair(usize::MAX, true));

    assert!(take_init_last(0));
    assert!(!take_init_last(1));
    assert!(!take_init_last(u64::MAX));

    assert_eq!(take_offset(false), OffsetKind::Buffered);
    assert_eq!(take_offset(true), OffsetKind::Reanchor);
}

#[test]
fn steps_update_cases() {
    assert_eq!(update(false, false), Update::None);
    assert_eq!(update(false, true), Update::Swap);
    assert_eq!(update(true, false), Update::Upload);
    assert_eq!(update(true, true), Update::Upload);
}

// ---------------------------------------------------------------------------
// The callers, copied from obs-source-deinterlace.c onto a stand-in source

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Frame {
    id: usize,
    timestamp: u64,
    prev_frame: bool,
}

#[derive(Debug, Default)]
struct Source {
    frames: Vec<Frame>,
    /// indices into `frames`, oldest first
    queue: Vec<usize>,
    last_frame_ts: u64,
    last_sys_timestamp: u64,
    async_unbuffered: bool,
    deinterlace_offset: u64,
    deinterlace_frame_ts: u64,
    deinterlace_half_duration: u32,
    prev_async_frame: Option<usize>,
    cur_async_frame: Option<usize>,
    steps: String,
}

const VIDEO_TIME: u64 = SYS + 3 * MS;
const INTERVAL: u64 = 16_666_667;

impl Source {
    /// n frames at T + i * step, ids 0 to n - 1, unflagged
    fn queued(n: usize, step: u64) -> Self {
        Self {
            frames: (0..n)
                .map(|i| Frame {
                    id: i,
                    timestamp: T + i as u64 * step,
                    prev_frame: false,
                })
                .collect(),
            queue: (0..n).collect(),
            last_sys_timestamp: SYS,
            ..Self::default()
        }
    }

    fn erase_front(&mut self) -> usize {
        let f = self.queue.remove(0);
        self.steps.push_str(&format!("E{f} "));
        f
    }

    fn remove_async_frame(&mut self, f: usize) {
        self.steps.push_str(&format!("R{f} "));
        self.frames[f].prev_frame = false;
    }

    fn set_flag(&mut self, f: usize, flag: bool) {
        self.steps.push_str(&format!("F{f}={} ", u8::from(flag)));
        self.frames[f].prev_frame = flag;
    }

    /// `ready_deinterlace_frames()`
    fn ready_frames(&mut self, sys_time: u64) -> bool {
        let timestamps: Vec<u64> = self
            .queue
            .iter()
            .map(|&f| self.frames[f].timestamp)
            .collect();
        let last_sys_timestamp = self.last_sys_timestamp;
        let async_unbuffered = self.async_unbuffered;
        let last_frame_ts = if async_unbuffered {
            0
        } else {
            self.last_frame_ts
        };
        let r = walk(
            &timestamps,
            sys_time,
            State {
                last_frame_ts,
                last_sys_timestamp,
                async_unbuffered,
            },
        );

        for _ in 0..r.drop_front {
            let dropped = self.erase_front();
            self.remove_async_frame(dropped);
        }

        if r.pairs {
            let offset = if self.async_unbuffered {
                self.deinterlace_offset
            } else {
                0
            };
            let pair = unbuffered_pair(
                timestamps[r.drop_front],
                timestamps[r.drop_front + 1],
                offset,
                sys_time,
            );
            if let Some(frame_ts) = pair.frame_ts {
                self.deinterlace_frame_ts = frame_ts;
            }
            self.set_flag(self.queue[0], pair.prev_frame);
        }

        self.last_frame_ts = r.last_frame_ts;
        if r.offset_reset {
            self.deinterlace_offset = 0;
        }
        if r.flag_front {
            self.set_flag(self.queue[0], true);
        }
        r.ready
    }

    /// `first_frame()`
    fn first_frame(&mut self) -> bool {
        let f = first(self.last_frame_ts, self.queue.len());
        if f.flag_front {
            self.set_flag(self.queue[0], true);
        }
        f.first
    }

    /// `deinterlace_get_closest_frames()`
    fn get_closest_frames(&mut self, sys_time: u64) {
        if self.async_unbuffered
            && self.deinterlace_offset != 0
            && deinterlace_pacing::hold(
                self.deinterlace_frame_ts,
                self.deinterlace_offset,
                self.deinterlace_half_duration,
                sys_time,
            )
        {
            self.steps.push_str("hold");
            return;
        }

        if self.queue.is_empty() {
            return;
        }

        if self.first_frame() || self.ready_frames(sys_time) {
            self.prev_async_frame = None;
            let mut cur = self.erase_front();
            self.cur_async_frame = Some(cur);

            let remaining = self.queue.len();
            let prev_frame = take_reads_flag(remaining) && self.frames[cur].prev_frame;

            if take_pair(remaining, prev_frame) {
                self.prev_async_frame = Some(cur);
                cur = self.erase_front();
                self.cur_async_frame = Some(cur);
                let prev = self.prev_async_frame.unwrap_or(cur);
                self.deinterlace_half_duration = deinterlace_pacing::half_duration(
                    self.frames[cur].timestamp,
                    self.frames[prev].timestamp,
                    self.deinterlace_half_duration,
                    INTERVAL,
                );
            } else {
                self.deinterlace_half_duration = deinterlace_pacing::half_duration(
                    self.frames[cur].timestamp,
                    self.deinterlace_frame_ts,
                    self.deinterlace_half_duration,
                    INTERVAL,
                );
            }

            if take_init_last(self.last_frame_ts) {
                self.last_frame_ts = self.frames[cur].timestamp;
            }

            self.deinterlace_frame_ts = self.frames[cur].timestamp;

            self.deinterlace_offset = match take_offset(self.async_unbuffered) {
                OffsetKind::Buffered => {
                    deinterlace_pacing::buffered_offset(VIDEO_TIME, self.last_frame_ts)
                }
                OffsetKind::Reanchor => deinterlace_pacing::reanchor(
                    self.deinterlace_offset,
                    VIDEO_TIME,
                    self.deinterlace_frame_ts,
                    self.deinterlace_half_duration,
                    INTERVAL,
                ),
            };
        }
    }
}

#[test]
fn steps_step_first_frame() {
    let mut s = Source::queued(1, S);
    s.get_closest_frames(SYS);
    assert_eq!(s.steps, "E0 ");
    assert_eq!(s.cur_async_frame, Some(0));
    assert_eq!(s.prev_async_frame, None);
    assert_eq!(s.last_frame_ts, T);
    assert_eq!(s.deinterlace_frame_ts, T);
    assert_eq!(s.deinterlace_offset, VIDEO_TIME - T + FRAME_TOL + TWOX);

    let mut s = Source::queued(2, S);
    s.get_closest_frames(SYS);
    assert_eq!(s.steps, "F0=1 E0 E1 ");
    assert_eq!(s.prev_async_frame, Some(0));
    assert_eq!(s.cur_async_frame, Some(1));
    assert_eq!(s.last_frame_ts, T + S);
    assert_eq!(u64::from(s.deinterlace_half_duration), S / 2);
}

#[test]
fn steps_step_buffered_drop_and_pair() {
    let mut s = Source::queued(5, S);
    s.last_frame_ts = T;
    s.deinterlace_offset = 77;
    s.get_closest_frames(SYS + 3 * S + 5 * MS);
    assert_eq!(s.steps, "E0 R0 E1 R1 F2=1 E2 E3 ");
    assert_eq!(s.prev_async_frame, Some(2));
    assert_eq!(s.cur_async_frame, Some(3));
    assert_eq!(s.queue.len(), 1);
    assert_eq!(s.last_frame_ts, T + 3 * S + 5 * MS);
    assert_eq!(
        s.deinterlace_offset,
        VIDEO_TIME - (T + 3 * S + 5 * MS) + FRAME_TOL + TWOX
    );
}

#[test]
fn steps_step_buffered_dropped_prev_is_no_pair() {
    let mut s = Source::queued(3, S);
    s.last_frame_ts = T;
    s.get_closest_frames(SYS + 100 * S);
    assert_eq!(s.steps, "E0 R0 E1 ");
    assert_eq!(s.cur_async_frame, Some(1));
    assert_eq!(s.prev_async_frame, None);
    assert_eq!(s.queue.len(), 1);

    // nothing due: nothing taken, the clock moves on
    let mut s = Source::queued(3, S);
    s.last_frame_ts = T - 10 * MS;
    s.get_closest_frames(SYS + 5 * MS);
    assert_eq!(s.steps, "");
    assert_eq!(s.queue.len(), 3);
    assert_eq!(s.last_frame_ts, T - 5 * MS);
}

#[test]
fn steps_step_unbuffered_past_max() {
    let mut s = Source::queued(33, S);
    s.async_unbuffered = true;
    s.last_frame_ts = T;
    s.get_closest_frames(SYS);
    let mut expected: String = (0..31).map(|i| format!("E{i} R{i} ")).collect();
    expected.push_str("F31=1 E31 E32 ");
    assert_eq!(s.steps, expected);
    assert_eq!(s.prev_async_frame, Some(31));
    assert_eq!(s.cur_async_frame, Some(32));
    assert_eq!(s.last_frame_ts, T + 31 * S);
    // no offset before: re-anchored at the current frame
    assert_eq!(s.deinterlace_offset, VIDEO_TIME - (T + 32 * S));
}

#[test]
fn steps_step_unbuffered_waits() {
    let waiting = || {
        let mut s = Source::queued(2, S);
        s.async_unbuffered = true;
        s.last_frame_ts = T - S;
        s.deinterlace_offset = SYS - T;
        s.deinterlace_frame_ts = T - 10 * S; // long past: no hold
        s
    };

    let mut s = waiting();
    s.get_closest_frames(SYS + 5 * MS);
    assert_eq!(s.steps, "F0=0 E0 ");
    assert_eq!(s.cur_async_frame, Some(0));
    assert_eq!(s.queue.len(), 1);
    assert_eq!(s.last_frame_ts, T);
    assert_eq!(s.deinterlace_frame_ts, T);
    // half the duration from the waiting frame_ts (T - S)
    assert_eq!(u64::from(s.deinterlace_half_duration), S / 2);

    // at the end: they pair
    let mut s = waiting();
    s.get_closest_frames(SYS + S);
    assert_eq!(s.steps, "F0=1 E0 E1 ");
    assert_eq!(s.prev_async_frame, Some(0));
    assert_eq!(s.cur_async_frame, Some(1));
}

#[test]
fn steps_step_update() {
    let step = |prev: bool, cur: bool| match update(prev, cur) {
        Update::Upload => "upload",
        Update::Swap => "swap",
        Update::None => "none",
    };
    assert_eq!(step(true, true), "upload");
    assert_eq!(step(true, false), "upload");
    assert_eq!(step(false, true), "swap");
    assert_eq!(step(false, false), "none");
}

// ---------------------------------------------------------------------------
// edge: the walk against the C before the move

/// `ready_deinterlace_frames()` of origin/master before the move, written
/// out: a queue of frames walked with a cursor and erased from the front
/// one by one, `last_frame_ts` and `deinterlace_offset` stored as the C
/// stored them, the flag set on the frame `prev_frame` ends on. `offset` is
/// the source's `deinterlace_offset`; the result carries the frame flags
/// and `deinterlace_frame_ts` it wrote too.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Before {
    erased: usize,
    last_frame_ts: u64,
    offset: u64,
    frame_ts: Option<u64>,
    /// (index in the queue left, flag) of each prev_frame write, in order
    flags: Vec<(usize, bool)>,
    ready: bool,
}

#[allow(clippy::too_many_arguments)]
fn ready_before(
    ts: &[u64],
    sys_time: u64,
    last_frame_ts: u64,
    last_sys_timestamp: u64,
    unbuffered: bool,
    offset: u64,
) -> Before {
    let mut q: Vec<u64> = ts.to_vec();
    let mut b = Before {
        erased: 0,
        last_frame_ts,
        offset,
        frame_ts: None,
        flags: Vec::new(),
        ready: false,
    };
    // next_frame, prev_frame and frame as positions in the original queue
    let mut next = 0usize;
    let sys_offset = sys_time.wrapping_sub(last_sys_timestamp);
    let mut frame_time = q[0];
    let mut frame_offset;

    if unbuffered {
        while q.len() > 2 {
            q.remove(0);
            b.erased += 1;
        }
        if q.len() == 2 {
            let mut has_prev = true;
            if b.offset != 0 && deinterlace_pacing::wait_for_next(q[0], q[1], b.offset, sys_time) {
                has_prev = false;
                b.frame_ts = Some(deinterlace_pacing::waiting_frame_ts(q[0], q[1]));
            }
            b.flags.push((0, has_prev));
        }
        b.last_frame_ts = q[0];
        b.ready = true;
        return b;
    }

    let oob = if frame_time < b.last_frame_ts {
        b.last_frame_ts - frame_time > MAX_VAR
    } else {
        frame_time - b.last_frame_ts > MAX_VAR
    };
    if oob {
        b.last_frame_ts = q[0];
        b.offset = 0;
        b.ready = true;
        return b;
    }
    frame_offset = frame_time.wrapping_sub(b.last_frame_ts);
    b.last_frame_ts = b.last_frame_ts.wrapping_add(sys_offset);

    // positions are in the original queue; the live queue starts at erased
    let mut prev: Option<usize> = None;
    let mut frame: Option<usize> = None;
    while b.last_frame_ts > ts[next] {
        if b.last_frame_ts - ts[next] < FRAME_TOL {
            break;
        }
        if let Some(p) = prev {
            assert_eq!(p, b.erased, "the erased prev_frame is the front frame");
            q.remove(0);
            b.erased += 1;
        }
        if q.len() <= 2 && !(frame.is_none() && q.len() == 2) {
            b.offset = 0;
            b.ready = true;
            return b;
        }
        let idx = if frame.is_some() { 2 } else { 1 };
        prev = frame;
        frame = Some(next);
        next = b.erased + idx;
        if ts[next].wrapping_sub(frame_time) > MAX_VAR {
            b.last_frame_ts = ts[next].wrapping_sub(frame_offset);
            b.offset = 0;
        }
        frame_time = ts[next];
        frame_offset = frame_time.wrapping_sub(b.last_frame_ts);
    }
    if let Some(p) = prev {
        b.flags.push((p - b.erased, true));
    }
    b.ready = frame.is_some();
    b
}

/// The walk and the pair step applied as `obs-source-deinterlace.c.h`
/// applies them.
fn ready_after(
    ts: &[u64],
    sys_time: u64,
    last_frame_ts: u64,
    last_sys_timestamp: u64,
    unbuffered: bool,
    offset: u64,
) -> Before {
    let r = walk(
        ts,
        sys_time,
        State {
            last_frame_ts: if unbuffered { 0 } else { last_frame_ts },
            last_sys_timestamp,
            async_unbuffered: unbuffered,
        },
    );
    let mut b = Before {
        erased: r.drop_front,
        last_frame_ts: r.last_frame_ts,
        offset,
        frame_ts: None,
        flags: Vec::new(),
        ready: r.ready,
    };
    if r.pairs {
        let pair = unbuffered_pair(
            ts[r.drop_front],
            ts[r.drop_front + 1],
            if unbuffered { offset } else { 0 },
            sys_time,
        );
        b.frame_ts = pair.frame_ts;
        b.flags.push((0, pair.prev_frame));
    }
    if r.offset_reset {
        b.offset = 0;
    }
    if r.flag_front {
        b.flags.push((0, true));
    }
    b
}

const EDGE_STEPS: [u64; 14] = [
    0,
    1,
    FRAME_TOL - 1,
    FRAME_TOL,
    FRAME_TOL + 1,
    S,
    MAX_VAR - 1,
    MAX_VAR,
    MAX_VAR + 1,
    u64::MAX - S + 1, // backwards by S
    u64::MAX,
    1 << 63,
    4 * MS,
    33_366_667,
];

/// Every queue of 0 to 6 frames on a regular step from the grid, every
/// clock on a grid around each frame, last_frame_ts at and around the
/// front frame and the out-of-bounds edges, buffered and unbuffered, with
/// and without an offset.
#[test]
fn edge_walk_matches_c_before_move_small_queues() {
    let mut runs = 0u64;
    for n in 0..=6usize {
        for &step in &EDGE_STEPS {
            let ts: Vec<u64> = (0..n as u64)
                .map(|i| T.wrapping_add(i.wrapping_mul(step)))
                .collect();
            let anchors = [
                0,
                T,
                T - 10 * MS,
                T + 3 * MS,
                T - MAX_VAR,
                T - MAX_VAR - 1,
                T + MAX_VAR,
                T + MAX_VAR + 1,
            ];
            for &last in &anchors {
                for k in 0..=n as u64 {
                    for &d in &[
                        0u64,
                        FRAME_TOL - 1,
                        FRAME_TOL,
                        FRAME_TOL + 1,
                        5 * MS,
                        u64::MAX,
                    ] {
                        let clock = T.wrapping_add(k.wrapping_mul(step)).wrapping_add(d);
                        let sys_time = SYS.wrapping_add(clock.wrapping_sub(last));
                        for unbuffered in [false, true] {
                            for offset in [0, SYS - T, 1] {
                                if n == 0 {
                                    let r = walk(
                                        &ts,
                                        sys_time,
                                        State {
                                            last_frame_ts: last,
                                            last_sys_timestamp: SYS,
                                            async_unbuffered: unbuffered,
                                        },
                                    );
                                    assert_eq!(r.drop_front, 0);
                                    assert!(!r.ready && !r.pairs && !r.flag_front);
                                    assert!(!r.offset_reset);
                                    assert_eq!(r.last_frame_ts, last);
                                    continue;
                                }
                                let before =
                                    ready_before(&ts, sys_time, last, SYS, unbuffered, offset);
                                let after =
                                    ready_after(&ts, sys_time, last, SYS, unbuffered, offset);
                                assert_eq!(
                                    after, before,
                                    "ts {ts:?} sys_time {sys_time} last {last} unbuffered {unbuffered} offset {offset}"
                                );
                                runs += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(runs > 100_000, "{runs}");
}

fn timestamps() -> impl Strategy<Value = Vec<u64>> {
    prop_oneof![
        // any u64s, unsorted
        prop::collection::vec(any::<u64>(), 1..40),
        // sorted
        prop::collection::vec(any::<u64>(), 1..40).prop_map(|mut v| {
            v.sort_unstable();
            v
        }),
        // a regular stream from any start, wrapping past u64::MAX
        (any::<u64>(), 0..3 * MAX_VAR, 1..40usize).prop_map(|(start, step, n)| {
            (0..n as u64)
                .map(|i| start.wrapping_add(i.wrapping_mul(step)))
                .collect()
        }),
        // a 50 fps stream with jitter, gaps and steps back
        (
            any::<u64>(),
            prop::collection::vec((0u8..8, 0..3 * MAX_VAR), 1..40)
        )
            .prop_map(|(start, steps)| {
                let mut t = start;
                steps
                    .into_iter()
                    .map(|(kind, d)| {
                        let v = t;
                        t = match kind {
                            0 => t.wrapping_add(d),
                            1 => t.wrapping_sub(d % S),
                            2 => t.wrapping_add(MAX_VAR + d % 3).wrapping_sub(1),
                            3 => t.wrapping_add(FRAME_TOL + d % 3).wrapping_sub(1),
                            _ => t.wrapping_add(S),
                        };
                        v
                    })
                    .collect()
            }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    /// Any timestamps, sorted or not, any clock, any state.
    #[test]
    fn edge_walk_matches_c_before_move(
        ts in timestamps(),
        last in prop_oneof![Just(0u64), any::<u64>(), Just(T)],
        near_front in any::<bool>(),
        delta in any::<u64>(),
        sys_offset in prop_oneof![any::<u64>(), 0..10 * S],
        unbuffered in any::<bool>(),
        offset in prop_oneof![Just(0u64), any::<u64>()],
    ) {
        // last_frame_ts near the front frame half the time, so the walk
        // runs rather than resetting
        let last = if near_front { ts[0].wrapping_sub(delta % (3 * S)) } else { last };
        let sys_time = SYS.wrapping_add(sys_offset);
        let before = ready_before(&ts, sys_time, last, SYS, unbuffered, offset);
        let after = ready_after(&ts, sys_time, last, SYS, unbuffered, offset);
        prop_assert_eq!(after, before);
    }

    /// The walk's bounds: drops at most count - 1, keeps a frame to take
    /// when ready, and an unbuffered walk keeps at most two.
    #[test]
    fn edge_walk_bounds(
        ts in prop::collection::vec(any::<u64>(), 0..40),
        sys_time in any::<u64>(),
        last_frame_ts in any::<u64>(),
        last_sys_timestamp in any::<u64>(),
        async_unbuffered in any::<bool>(),
    ) {
        let r = walk(&ts, sys_time, State { last_frame_ts, last_sys_timestamp, async_unbuffered });
        if ts.is_empty() {
            prop_assert_eq!(r.drop_front, 0);
            prop_assert!(!r.ready);
        } else {
            prop_assert!(r.drop_front < ts.len());
        }
        if async_unbuffered && !ts.is_empty() {
            prop_assert_eq!(ts.len() - r.drop_front, ts.len().min(2));
            prop_assert_eq!(r.pairs, ts.len() >= 2);
            prop_assert!(!r.flag_front && !r.offset_reset);
        } else {
            prop_assert!(!r.pairs);
        }
        if r.flag_front {
            // a pair needs the next frame
            prop_assert!(ts.len() - r.drop_front >= 2 && r.ready);
        }
    }

    /// The pair guard against `deinterlace_pacing`, for any input.
    #[test]
    fn edge_unbuffered_pair(ts in any::<u64>(), next in any::<u64>(), offset in any::<u64>(), sys_time in any::<u64>()) {
        let waits = offset != 0 && sys_time < ts.wrapping_add(offset).wrapping_add(next.wrapping_sub(ts));
        let r = unbuffered_pair(ts, next, offset, sys_time);
        prop_assert_eq!(r.prev_frame, !waits);
        prop_assert_eq!(r.frame_ts, waits.then(|| ts.wrapping_sub(next.wrapping_sub(ts))));
    }

    /// first() for any input.
    #[test]
    fn edge_first(last_frame_ts in prop_oneof![Just(0u64), any::<u64>()], count in any::<usize>()) {
        prop_assert_eq!(first(last_frame_ts, count), First {
            first: last_frame_ts == 0,
            flag_front: last_frame_ts == 0 && count >= 2,
        });
    }
}

/// The small decisions over every boolean input and the size and timestamp
/// edges.
#[test]
fn edge_small_decisions() {
    for remaining in [0usize, 1, 2, usize::MAX / 2, usize::MAX] {
        assert_eq!(take_reads_flag(remaining), remaining != 0);
        for flag in [false, true] {
            assert_eq!(take_pair(remaining, flag), remaining != 0 && flag);
        }
    }
    for last in [0u64, 1, T, u64::MAX] {
        assert_eq!(take_init_last(last), last == 0);
    }
    for has_prev in [false, true] {
        for updated in [false, true] {
            let expected = if has_prev {
                Update::Upload
            } else if updated {
                Update::Swap
            } else {
                Update::None
            };
            assert_eq!(update(has_prev, updated), expected);
        }
    }
}
