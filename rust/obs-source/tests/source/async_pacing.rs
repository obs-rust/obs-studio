//! Tier 1: `obs_source::async_pacing`, the async video frame pacing.
//!
//! The `steps_*` cases port `test/cmocka/test_async_frame_pacing_steps.c`
//! 1:1, under the same case names, in the same order, with the same inputs
//! and expectations, the model queue of the sequences included.
//!
//! The `edge_*` cases are new. They check [`pace`] against a model of the
//! original `get_closest_frame()` and `ready_async_frame()` written as the
//! C was, on a `Vec` queue that erases at index 0 and records each frame it
//! releases: over every queue of up to three special values of the `u64`
//! range and the thresholds +-1, and proptests over full-range queues of 1
//! to 30 frames and over frame-rate-like queues. They also check the
//! contract `obs-source.c.h` relies on (`drop_front` below the queue length,
//! nothing dropped without a frame taken) and [`frame_out_of_bounds`]
//! against `i128` arithmetic.

use obs_source::async_pacing::{
    FRAME_TOLERANCE, MAX_TS_VAR, Pacing, State, frame_out_of_bounds, pace,
};
use proptest::prelude::*;

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
const FRAME_TOL: u64 = FRAME_TOLERANCE;
/// A source clock well away from 0 and `u64::MAX`.
const L: u64 = 100 * SEC;
/// The canvas clock (`obs->video.video_time`), on another base.
const VT: u64 = 5000 * SEC;
const QUEUE_MAX: usize = 64;

#[track_caller]
#[allow(clippy::too_many_arguments)]
fn check_pace(
    ts: &[u64],
    sys_time: u64,
    last_frame_ts: u64,
    last_sys_timestamp: u64,
    unbuffered: bool,
    drop_front: usize,
    take_first: bool,
    new_last_frame_ts: u64,
) {
    let state = State {
        last_frame_ts,
        last_sys_timestamp,
        async_unbuffered: unbuffered,
    };
    let r = pace(ts, sys_time, state);
    assert_eq!(
        r,
        Pacing {
            drop_front,
            take_first,
            last_frame_ts: new_last_frame_ts,
        },
        "pace({ts:?}, {sys_time}, {state:?})"
    );
    check_contract(ts, r);
}

/// What `get_closest_frame()` relies on.
#[track_caller]
fn check_contract(ts: &[u64], r: Pacing) {
    if !ts.is_empty() {
        assert!(r.drop_front < ts.len(), "drop_front {r:?} for {ts:?}");
    }
    if !r.take_first {
        assert_eq!(r.drop_front, 0, "dropped without a frame taken: {r:?}");
    }
}

/// `async_frames`, `last_frame_ts`, `last_sys_timestamp` and
/// `async_unbuffered` of a source, and what `async_tick()` did with them.
#[derive(Default)]
struct Model {
    ts: Vec<u64>,
    last_frame_ts: u64,
    last_sys_timestamp: u64,
    unbuffered: bool,
    shown: usize,
    dropped: usize,
    empty_ticks: usize,
}

impl Model {
    fn push(&mut self, ts: u64) {
        assert!(self.ts.len() < QUEUE_MAX);
        self.ts.push(ts);
    }

    /// `get_closest_frame()` and the store of `last_sys_timestamp` in
    /// `async_tick()`: the timestamp of the frame taken.
    fn tick(&mut self, sys_time: u64) -> Option<u64> {
        let mut taken = None;
        if !self.ts.is_empty() {
            let r = pace(
                &self.ts,
                sys_time,
                State {
                    last_frame_ts: self.last_frame_ts,
                    last_sys_timestamp: self.last_sys_timestamp,
                    async_unbuffered: self.unbuffered,
                },
            );
            assert!(r.drop_front < self.ts.len());
            for _ in 0..r.drop_front {
                self.ts.remove(0);
                self.dropped += 1;
            }
            self.last_frame_ts = r.last_frame_ts;
            if r.take_first {
                taken = Some(self.ts.remove(0));
                self.shown += 1;
            }
        }
        if taken.is_none() {
            self.empty_ticks += 1;
        }
        self.last_sys_timestamp = sys_time;
        taken
    }
}

// ---------------------------------------------------------------------------

#[test]
fn steps_empty_queue() {
    check_pace(&[], VT, L, VT, false, 0, false, L);
    check_pace(&[], VT, L, VT, true, 0, false, L);
    check_pace(&[], VT, 0, 0, false, 0, false, 0);
}

#[test]
fn steps_first_frame_rule() {
    let ts = [L, L + 33 * MS, L + 66 * MS];
    check_pace(&ts, VT, 0, VT - 16 * MS, false, 0, true, L);
    check_pace(&ts, VT, 0, VT - 16 * MS, true, 0, true, L);
    check_pace(&ts[..1], 0, 0, u64::MAX, false, 0, true, L);

    // Characterized, not endorsed: a frame at timestamp 0 seeds 0.
    let zero = [0, 33 * MS];
    check_pace(&zero, VT, 0, VT, false, 0, true, 0);
    check_pace(&zero[1..], VT, 0, VT, false, 0, true, 33 * MS);
    check_pace(&zero, VT, 0, VT, true, 0, true, 0);
}

#[test]
fn steps_unbuffered_newest() {
    let ts: Vec<u64> = (0..30).map(|i| L + i * 33 * MS).collect();
    check_pace(&ts[..1], VT, L - SEC, VT, true, 0, true, L);
    check_pace(&ts[..2], VT, L - SEC, VT, true, 1, true, L + 33 * MS);
    check_pace(&ts, VT, L - SEC, VT, true, 29, true, L + 29 * 33 * MS);
    check_pace(&ts, 0, 1, u64::MAX, true, 29, true, L + 29 * 33 * MS);

    check_pace(&[L, L + 100 * SEC], VT, L, VT, true, 1, true, L + 100 * SEC);
    check_pace(&[L, 5 * MS], VT, L, VT, true, 1, true, 5 * MS);

    check_pace(&[L, 0], VT, L, VT, true, 1, true, 0);
}

#[test]
fn steps_unbuffered_more_than_max_frames() {
    let ts: Vec<u64> = (0..40).map(|i| L + i * 10 * MS).collect();
    check_pace(&ts[..31], VT, L, VT, true, 30, true, L + 300 * MS);
    check_pace(&ts, VT, L, VT, true, 39, true, L + 390 * MS);
    check_pace(&ts, VT + SEC, L, VT, false, 39, true, L + SEC);
}

#[test]
fn steps_timing_jump_edges() {
    let lst = VT - 16 * MS;

    check_pace(
        &[L + MAX_TS_VAR + 1],
        VT,
        L,
        lst,
        false,
        0,
        true,
        L + MAX_TS_VAR + 1,
    );
    check_pace(&[L + MAX_TS_VAR], VT, L, lst, false, 0, false, L + 16 * MS);
    check_pace(
        &[L + MAX_TS_VAR - 1],
        VT,
        L,
        lst,
        false,
        0,
        false,
        L + 16 * MS,
    );

    check_pace(
        &[L - MAX_TS_VAR - 1],
        VT,
        L,
        lst,
        false,
        0,
        true,
        L - MAX_TS_VAR - 1,
    );
    check_pace(&[L - MAX_TS_VAR], VT, L, lst, false, 0, true, L + 16 * MS);
    check_pace(
        &[L - MAX_TS_VAR + 1],
        VT,
        L,
        lst,
        false,
        0,
        true,
        L + 16 * MS,
    );

    let jump3 = [L + 3 * SEC, L + 3 * SEC + 33 * MS, L + 10 * SEC];
    check_pace(&jump3, VT, L, lst, false, 0, true, L + 3 * SEC);

    check_pace(&[u64::MAX], VT, 1, lst, false, 0, true, u64::MAX);
    check_pace(&[0], VT, u64::MAX, lst, false, 0, true, 0);
}

#[test]
fn steps_duplicate_guard_edges() {
    let first = L - 50 * MS;
    check_pace(&[first, L - 1_999_999], VT, L, VT, false, 0, true, L);
    check_pace(&[first, L - 2_000_000], VT, L, VT, false, 1, true, L);
    check_pace(&[first, L - 2_000_001], VT, L, VT, false, 1, true, L);

    check_pace(&[L - 1, L + 33 * MS], VT, L, VT, false, 0, true, L);
    check_pace(&[L, L + 33 * MS], VT, L, VT, false, 0, false, L);

    let four = [L - 90 * MS, L - 60 * MS, L - 30 * MS, L - MS];
    check_pace(&four, VT, L, VT, false, 2, true, L);
}

#[test]
fn steps_reanchor_in_walk() {
    let a = L - 10 * MS;
    let mut ts = [a, a + MAX_TS_VAR + 1, a + MAX_TS_VAR + 1 + 33 * MS];
    check_pace(&ts[..2], VT, L, VT, false, 1, true, L + MAX_TS_VAR + 1);
    check_pace(&ts, VT, L, VT, false, 1, true, L + MAX_TS_VAR + 1);

    ts[1] = a + MAX_TS_VAR;
    ts[2] = a + MAX_TS_VAR + 33 * MS;
    check_pace(&ts, VT, L, VT, false, 0, true, L);

    check_pace(&[a, a - SEC], VT, L, VT, false, 1, true, L - SEC);

    // Characterized, not endorsed: the re-anchor drops this tick's 20 ms.
    let twice = [a, a + 3 * SEC, a + 6 * SEC, a + 6 * SEC + 50 * MS];
    check_pace(&twice, VT + 20 * MS, L, VT, false, 2, true, L + 6 * SEC);

    // Characterized, not endorsed: a re-anchor onto 0 re-seeds.
    check_pace(&[a, u64::MAX - 10 * MS + 1], VT, L, VT, false, 0, true, a);
}

#[test]
fn steps_timestamps_backwards() {
    let a = L - 10 * MS;
    check_pace(&[a, a - 1, a + 33 * MS], VT, L, VT, false, 1, true, L - 1);

    let descending = [L - 10 * MS, L - 20 * MS, L - 30 * MS, L - 40 * MS];
    check_pace(&descending, VT, L, VT, false, 3, true, L - 30 * MS);

    check_pace(&[L - SEC], VT, L, VT, false, 0, true, L);
}

#[test]
fn steps_sys_time_backwards() {
    let ts = [L - 50 * MS];
    check_pace(&ts, VT - 100 * MS, L, VT, false, 0, false, L - 100 * MS);
    check_pace(&ts, VT - 40 * MS, L, VT, false, 0, true, L - 40 * MS);

    let small = [40 * MS, 73 * MS, 106 * MS];
    check_pace(
        &small,
        VT - 100 * MS,
        50 * MS,
        VT,
        false,
        2,
        true,
        0u64.wrapping_sub(50 * MS),
    );

    let mut m = Model {
        last_frame_ts: 50 * MS,
        last_sys_timestamp: VT,
        ..Model::default()
    };
    m.push(40 * MS);
    m.push(73 * MS);
    m.push(106 * MS);
    assert_eq!(m.tick(VT - 100 * MS), Some(106 * MS));
    assert_eq!(m.last_frame_ts, 0u64.wrapping_sub(50 * MS));
    m.push(139 * MS);
    assert_eq!(m.tick(VT - 84 * MS), Some(139 * MS));
    assert_eq!(m.last_frame_ts, 139 * MS);
    assert_eq!(m.dropped, 2);

    check_pace(&ts, VT - L, L, VT, false, 0, false, 0);
    check_pace(&ts, VT, 0, VT - L, false, 0, true, L - 50 * MS);
}

#[test]
fn steps_all_frames_in_the_past() {
    let mut ts: Vec<u64> = (0..30).map(|i| L - SEC + i * 33 * MS).collect();
    check_pace(&ts, VT, L, VT, false, 29, true, L);

    ts[29] = L - MS;
    check_pace(&ts, VT, L, VT, false, 28, true, L);

    ts[29] = L + 15 * MS;
    check_pace(&ts, VT + 16 * MS, L, VT, false, 28, true, L + 16 * MS);
}

/// `run_sequence()` of the C test: exact integer clocks, each frame queued
/// one source interval before the canvas reaches it.
fn run_sequence(
    m: &mut Model,
    src: u64,
    num: u64,
    den: u64,
    tick_num: u64,
    tick_den: u64,
    ticks: u64,
) {
    let mut queued = 0;
    for k in 0..ticks {
        let sys_time = VT + k * tick_num * SEC / tick_den;
        while src + queued * num * SEC / den <= src + (sys_time - VT) + num * SEC / den {
            m.push(src + queued * num * SEC / den);
            queued += 1;
        }
        let _ = m.tick(sys_time);
    }
}

#[test]
fn steps_seq_buffered_rates() {
    let mut m = Model {
        last_sys_timestamp: VT,
        ..Model::default()
    };
    run_sequence(&mut m, L, 1, 30, 1, 60, 120);
    assert_eq!(
        (m.shown, m.dropped, m.empty_ticks),
        (60, 0, 60),
        "30 into 60"
    );

    let mut m2 = Model {
        last_sys_timestamp: VT,
        ..Model::default()
    };
    run_sequence(&mut m2, L, 1, 60, 1, 30, 60);
    assert_eq!(
        (m2.shown, m2.dropped, m2.empty_ticks),
        (60, 58, 0),
        "60 into 30"
    );

    let mut m3 = Model {
        last_sys_timestamp: VT,
        ..Model::default()
    };
    run_sequence(&mut m3, L, 1, 30, 1, 30, 60);
    assert_eq!(
        (m3.shown, m3.dropped, m3.empty_ticks),
        (59, 0, 1),
        "30 into 30"
    );
}

#[test]
fn steps_seq_unbuffered_rates() {
    let mut m = Model {
        last_sys_timestamp: VT,
        unbuffered: true,
        ..Model::default()
    };
    run_sequence(&mut m, L, 1, 60, 1, 30, 60);
    assert_eq!((m.shown, m.dropped), (60, 59));
    assert_eq!(m.last_frame_ts, L + 118 * SEC / 60);
}

// --- edge cases ------------------------------------------------------------

/// The original `get_closest_frame()` and `ready_async_frame()`, written as
/// the C was, on a queue that erases at index 0 (`da_erase(..., 0)`) and
/// releases frames by their position in the original queue
/// (`remove_async_frame()`, `None` for the walk's NULL frame). Returns the
/// released positions, the taken position and the new `last_frame_ts`.
struct OriginalModel {
    /// (position in the original queue, timestamp)
    queue: Vec<(usize, u64)>,
    last_frame_ts: u64,
    released: Vec<usize>,
}

impl OriginalModel {
    fn remove_async_frame(&mut self, frame: Option<(usize, u64)>) {
        if let Some((pos, _)) = frame {
            self.released.push(pos);
        }
    }

    fn out_of_bounds(&self, ts: u64) -> bool {
        if ts < self.last_frame_ts {
            self.last_frame_ts - ts > MAX_TS_VAR
        } else {
            ts - self.last_frame_ts > MAX_TS_VAR
        }
    }

    fn ready(&mut self, sys_time: u64, last_sys_timestamp: u64, unbuffered: bool) -> bool {
        let mut next_frame = self.queue[0];
        let mut frame: Option<(usize, u64)> = None;
        let sys_offset = sys_time.wrapping_sub(last_sys_timestamp);
        let mut frame_time = next_frame.1;
        let mut frame_offset;

        if unbuffered {
            while self.queue.len() > 1 {
                self.queue.remove(0);
                self.remove_async_frame(Some(next_frame));
                next_frame = self.queue[0];
            }
            self.last_frame_ts = next_frame.1;
            return true;
        }

        if self.out_of_bounds(frame_time) {
            self.last_frame_ts = next_frame.1;
            return true;
        }
        frame_offset = frame_time.wrapping_sub(self.last_frame_ts);
        self.last_frame_ts = self.last_frame_ts.wrapping_add(sys_offset);

        while self.last_frame_ts > next_frame.1 {
            if frame.is_some() && self.last_frame_ts.wrapping_sub(next_frame.1) < 2_000_000 {
                break;
            }
            if frame.is_some() {
                self.queue.remove(0);
            }
            self.remove_async_frame(frame);
            if self.queue.len() == 1 {
                return true;
            }
            frame = Some(next_frame);
            next_frame = self.queue[1];
            if next_frame.1.wrapping_sub(frame_time) > 2_000_000_000 {
                self.last_frame_ts = next_frame.1.wrapping_sub(frame_offset);
            }
            frame_time = next_frame.1;
            frame_offset = frame_time.wrapping_sub(self.last_frame_ts);
        }
        frame.is_some()
    }

    /// `get_closest_frame()`: the position taken.
    fn closest(
        &mut self,
        sys_time: u64,
        last_sys_timestamp: u64,
        unbuffered: bool,
    ) -> Option<usize> {
        if self.queue.is_empty() {
            return None;
        }
        if self.last_frame_ts == 0 || self.ready(sys_time, last_sys_timestamp, unbuffered) {
            let frame = self.queue.remove(0);
            if self.last_frame_ts == 0 {
                self.last_frame_ts = frame.1;
            }
            return Some(frame.0);
        }
        None
    }
}

/// The original's result as a [`Pacing`], after checking that its erases
/// were a prefix of the queue, released in order, and that the frame taken
/// was the one after them.
fn original(ts: &[u64], sys_time: u64, state: State) -> Pacing {
    let mut m = OriginalModel {
        queue: ts.iter().copied().enumerate().collect(),
        last_frame_ts: state.last_frame_ts,
        released: Vec::new(),
    };
    let taken = m.closest(sys_time, state.last_sys_timestamp, state.async_unbuffered);
    let drop_front = m.released.len();
    assert_eq!(
        m.released,
        (0..drop_front).collect::<Vec<_>>(),
        "non-prefix erase"
    );
    if let Some(pos) = taken {
        assert_eq!(pos, drop_front, "taken frame not the front");
    }
    Pacing {
        drop_front,
        take_first: taken.is_some(),
        last_frame_ts: m.last_frame_ts,
    }
}

#[track_caller]
fn check_against_original(ts: &[u64], sys_time: u64, state: State) {
    let r = pace(ts, sys_time, state);
    assert_eq!(
        r,
        original(ts, sys_time, state),
        "pace({ts:?}, {sys_time}, {state:?})"
    );
    check_contract(ts, r);
    if !ts.is_empty() && state.last_frame_ts == 0 {
        assert_eq!((r.drop_front, r.take_first), (0, true), "first-frame rule");
    }
    if !ts.is_empty() && state.last_frame_ts != 0 && state.async_unbuffered {
        assert_eq!(
            (r.drop_front, r.take_first),
            (ts.len() - 1, true),
            "newest only"
        );
    }
}

/// Special values of the `u64` range and of the thresholds, around `L`.
fn specials() -> Vec<u64> {
    vec![
        0,
        1,
        FRAME_TOL - 1,
        FRAME_TOL,
        MAX_TS_VAR,
        MAX_TS_VAR + 1,
        L - MAX_TS_VAR - 1,
        L - MAX_TS_VAR,
        L - FRAME_TOL,
        L - 1,
        L,
        L + 1,
        L + FRAME_TOL,
        L + MAX_TS_VAR,
        L + MAX_TS_VAR + 1,
        1 << 63,
        u64::MAX - MAX_TS_VAR,
        u64::MAX,
    ]
}

/// (last_sys_timestamp, sys_time) pairs: no time, a tick, the guard edges,
/// the jump edges and the clock going backwards.
fn clock_pairs() -> Vec<(u64, u64)> {
    vec![
        (VT, VT),
        (VT, VT + 16_666_666),
        (VT, VT + FRAME_TOL - 1),
        (VT, VT + FRAME_TOL),
        (VT, VT + MAX_TS_VAR + 1),
        (VT, VT - 1),
        (VT, VT - 100 * MS),
        (VT, 0),
        (0, u64::MAX),
    ]
}

#[test]
fn edge_frame_out_of_bounds() {
    for &lft in &specials() {
        for &ts in &specials() {
            let expected = (i128::from(ts) - i128::from(lft)).abs() > i128::from(MAX_TS_VAR);
            assert_eq!(frame_out_of_bounds(lft, ts), expected, "({lft}, {ts})");
        }
    }
    // the edges either way
    assert!(!frame_out_of_bounds(L, L + MAX_TS_VAR));
    assert!(frame_out_of_bounds(L, L + MAX_TS_VAR + 1));
    assert!(!frame_out_of_bounds(L, L - MAX_TS_VAR));
    assert!(frame_out_of_bounds(L, L - MAX_TS_VAR - 1));
}

#[test]
fn edge_special_queues_of_one_and_two() {
    let sp = specials();
    for &lft in &sp {
        for &(lst, sys) in &clock_pairs() {
            for unbuffered in [false, true] {
                let state = State {
                    last_frame_ts: lft,
                    last_sys_timestamp: lst,
                    async_unbuffered: unbuffered,
                };
                check_against_original(&[], sys, state);
                for &a in &sp {
                    check_against_original(&[a], sys, state);
                    for &b in &sp {
                        check_against_original(&[a, b], sys, state);
                    }
                }
            }
        }
    }
}

#[test]
fn edge_special_queues_of_three() {
    let sp = specials();
    for &lft in &[0, 1, L, u64::MAX] {
        for &(lst, sys) in &clock_pairs()[..4] {
            let state = State {
                last_frame_ts: lft,
                last_sys_timestamp: lst,
                async_unbuffered: false,
            };
            for &a in &sp {
                for &b in &sp {
                    for &c in &sp {
                        check_against_original(&[a, b, c], sys, state);
                    }
                }
            }
        }
    }
}

fn state_strategy() -> impl Strategy<Value = State> {
    (
        prop_oneof![Just(0u64), any::<u64>(), (L - 3 * SEC)..(L + 3 * SEC)],
        any::<u64>(),
        any::<bool>(),
    )
        .prop_map(
            |(last_frame_ts, last_sys_timestamp, async_unbuffered)| State {
                last_frame_ts,
                last_sys_timestamp,
                async_unbuffered,
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    #[test]
    fn edge_full_range_queues(
        ts in prop::collection::vec(any::<u64>(), 1..=30),
        sys_time in any::<u64>(),
        state in state_strategy(),
    ) {
        check_against_original(&ts, sys_time, state);
    }

    /// Frame-rate-like queues near the clock: a base, a step from 1 ms to
    /// 100 ms with jitter, sometimes a jump, with the canvas a tick or
    /// two on.
    #[test]
    fn edge_paced_queues(
        start in (L - 2 * SEC)..(L + 100 * MS),
        step in MS..(100 * MS),
        jitter in prop::collection::vec(0u64..(4 * MS), 1..=30),
        jump_at in 0usize..40,
        jump in prop_oneof![Just(MAX_TS_VAR), Just(MAX_TS_VAR + 1), any::<u64>()],
        advance in 0u64..(50 * MS),
        unbuffered in any::<bool>(),
    ) {
        let mut t = start;
        let ts: Vec<u64> = jitter
            .iter()
            .enumerate()
            .map(|(i, j)| {
                if i == jump_at {
                    t = t.wrapping_add(jump);
                }
                let v = t.wrapping_add(*j).wrapping_sub(2 * MS);
                t = t.wrapping_add(step);
                v
            })
            .collect();
        let state = State {
            last_frame_ts: L,
            last_sys_timestamp: VT,
            async_unbuffered: unbuffered,
        };
        check_against_original(&ts, VT.wrapping_add(advance), state);
    }
}
