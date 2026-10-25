//! Tier 3: the `deinterlace_select` shims against the original C
//! (`obs-c-oracle`, `oracle/source_deinterlace_select.c`: the unmodified
//! `libobs/obs-source-deinterlace-select.c.h` with its functions renamed,
//! calling the C pacing of `oracle/source_deinterlace_pacing.c`).
//!
//! Every input goes to the oracle, the shim and the safe core. The results
//! are compared as the bytes the C lays out: `drop_front` and the
//! timestamps as their little-endian bytes, each `bool` as the byte the C
//! stores (0 or 1, and equal), the enums as the `u32` the C returns.
//!
//! Runs:
//! - the header's constant, enum values, struct layouts and enum sizes;
//! - the walk exhaustively over queues of 0 to 6 frames x the state flags
//!   (buffered or unbuffered, `last_frame_ts` zero, at, around or far from
//!   the front frame) on 16 steps (the `FRAME_TOLERANCE` and `MAX_TS_VAR`
//!   edges, backwards, wrapping, 0, 1, 2^63, `u64::MAX`) and a clock at and
//!   around each frame;
//! - proptests (20,000 cases each) over full-range `u64` timestamp vectors
//!   of 0 to 40 frames (sorted, unsorted, regular from any start and
//!   wrapping, jittered with steps back and threshold gaps) and any state;
//! - `first()`, the pair guard and the take and update decisions over
//!   their edges exhaustively and any input by proptest;
//! - sequences through a model of `async_tick()`'s deinterlaced path
//!   (`deinterlace_process_last_frame()`, `deinterlace_get_closest_frames()`,
//!   `first_frame()` and `ready_deinterlace_frames()` as
//!   `obs-source-deinterlace.c.h` now calls them, with the queue, its erases
//!   and releases, the flags and the source's fields), one copy driven by
//!   the C and one by the shims: 50i and 60i into 30, 50 and 60 fps
//!   canvases, buffered and unbuffered, with jitter, bursts, stalls, jumps
//!   and steps back. The steps and the state after every tick must match.
//!
//! MSVC x64: nothing here divides or reaches `util_mul_div64()`, so no input
//! needs a guard. Intentional difference (not compared): a null
//! `timestamps` with a count above 0 is an empty queue for the shim; the C
//! reads through it.

use obs_c_oracle::source_deinterlace_select as oracle;
use obs_source::deinterlace_pacing;
use obs_source::deinterlace_select::{self as core, FRAME_TOLERANCE, MAX_TS_VAR, State};
use obs_source::ffi::source_deinterlace_select as shim;
use proptest::prelude::*;

const MS: u64 = 1_000_000;
const T: u64 = 1_000_000_000_000;
const S: u64 = 20 * MS;
const SYS: u64 = 7_000_000_000_000;

/// A walk result as the C lays it out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WalkOut {
    drop_front: usize,
    last_frame_ts: u64,
    offset_reset: u8,
    pairs: u8,
    flag_front: u8,
    ready: u8,
}

fn ptr(ts: &[u64]) -> *const u64 {
    if ts.is_empty() {
        std::ptr::null()
    } else {
        ts.as_ptr()
    }
}

fn walk_c(ts: &[u64], sys_time: u64, last: u64, last_sys: u64, unbuffered: bool) -> WalkOut {
    // SAFETY: ts.len() readable timestamps (or null for none), not written
    // during the call.
    let r = unsafe {
        oracle::oracle_obs_source_deinterlace_select_walk(
            ptr(ts),
            ts.len(),
            sys_time,
            last,
            last_sys,
            unbuffered,
        )
    };
    WalkOut {
        drop_front: r.drop_front,
        last_frame_ts: r.last_frame_ts,
        offset_reset: r.offset_reset,
        pairs: r.pairs,
        flag_front: r.flag_front,
        ready: r.ready,
    }
}

fn walk_shim(ts: &[u64], sys_time: u64, last: u64, last_sys: u64, unbuffered: bool) -> WalkOut {
    // SAFETY: as walk_c.
    let r = unsafe {
        shim::obs_source_deinterlace_select_walk(
            ptr(ts),
            ts.len(),
            sys_time,
            last,
            last_sys,
            unbuffered,
        )
    };
    WalkOut {
        drop_front: r.drop_front,
        last_frame_ts: r.last_frame_ts,
        offset_reset: u8::from(r.offset_reset),
        pairs: u8::from(r.pairs),
        flag_front: u8::from(r.flag_front),
        ready: u8::from(r.ready),
    }
}

fn walk_core(ts: &[u64], sys_time: u64, last: u64, last_sys: u64, unbuffered: bool) -> WalkOut {
    let r = core::walk(
        ts,
        sys_time,
        State {
            last_frame_ts: last,
            last_sys_timestamp: last_sys,
            async_unbuffered: unbuffered,
        },
    );
    WalkOut {
        drop_front: r.drop_front,
        last_frame_ts: r.last_frame_ts,
        offset_reset: u8::from(r.offset_reset),
        pairs: u8::from(r.pairs),
        flag_front: u8::from(r.flag_front),
        ready: u8::from(r.ready),
    }
}

fn check_walk(ts: &[u64], sys_time: u64, last: u64, last_sys: u64, unbuffered: bool) -> WalkOut {
    let c = walk_c(ts, sys_time, last, last_sys, unbuffered);
    let s = walk_shim(ts, sys_time, last, last_sys, unbuffered);
    let k = walk_core(ts, sys_time, last, last_sys, unbuffered);
    assert!(
        c.offset_reset <= 1 && c.pairs <= 1 && c.flag_front <= 1 && c.ready <= 1,
        "{c:?}"
    );
    assert_eq!(
        s, c,
        "shim: ts {ts:?} sys_time {sys_time} last {last} last_sys {last_sys} unbuffered {unbuffered}"
    );
    assert_eq!(
        k, c,
        "core: ts {ts:?} sys_time {sys_time} last {last} last_sys {last_sys} unbuffered {unbuffered}"
    );
    c
}

fn pair_c(ts: u64, next: u64, offset: u64, sys_time: u64) -> (u8, u8, u64) {
    // SAFETY: plain values.
    let r = unsafe {
        oracle::oracle_obs_source_deinterlace_select_unbuffered_pair(ts, next, offset, sys_time)
    };
    (r.prev_frame, r.set_frame_ts, r.frame_ts)
}

fn check_pair(ts: u64, next: u64, offset: u64, sys_time: u64) -> (u8, u8, u64) {
    let c = pair_c(ts, next, offset, sys_time);
    let r = shim::obs_source_deinterlace_select_unbuffered_pair(ts, next, offset, sys_time);
    assert_eq!(
        (u8::from(r.prev_frame), u8::from(r.set_frame_ts), r.frame_ts),
        c,
        "{ts} {next} {offset} {sys_time}"
    );
    let k = core::unbuffered_pair(ts, next, offset, sys_time);
    assert_eq!(
        (
            u8::from(k.prev_frame),
            u8::from(k.frame_ts.is_some()),
            k.frame_ts.unwrap_or(0)
        ),
        c
    );
    c
}

fn check_first(last: u64, count: usize) {
    // SAFETY: plain values.
    let c = unsafe { oracle::oracle_obs_source_deinterlace_select_first(last, count) };
    let s = shim::obs_source_deinterlace_select_first(last, count);
    let k = core::first(last, count);
    assert_eq!(
        (u8::from(s.first), u8::from(s.flag_front)),
        (c.first, c.flag_front)
    );
    assert_eq!(
        (u8::from(k.first), u8::from(k.flag_front)),
        (c.first, c.flag_front)
    );
}

#[test]
fn constants_and_layouts() {
    let rust = [
        MAX_TS_VAR,
        u64::from(u32::from(core::OffsetKind::Buffered)),
        u64::from(u32::from(core::OffsetKind::Reanchor)),
        u64::from(u32::from(core::Update::None)),
        u64::from(u32::from(core::Update::Upload)),
        u64::from(u32::from(core::Update::Swap)),
    ];
    for (i, (name, value)) in oracle::CONSTANT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: plain value.
        assert_eq!(
            unsafe { oracle::oracle_source_deinterlace_select_constant(i) },
            value,
            "{name}"
        );
    }
    assert_eq!(FRAME_TOLERANCE, deinterlace_pacing::FRAME_TOLERANCE);

    use shim::{
        obs_source_deinterlace_select_first_result as F,
        obs_source_deinterlace_select_pair_result as P,
        obs_source_deinterlace_select_walk_result as W,
    };
    let layout = [
        size_of::<F>(),
        std::mem::offset_of!(F, first),
        std::mem::offset_of!(F, flag_front),
        size_of::<W>(),
        std::mem::offset_of!(W, drop_front),
        std::mem::offset_of!(W, last_frame_ts),
        std::mem::offset_of!(W, offset_reset),
        std::mem::offset_of!(W, pairs),
        std::mem::offset_of!(W, flag_front),
        std::mem::offset_of!(W, ready),
        size_of::<P>(),
        std::mem::offset_of!(P, prev_frame),
        std::mem::offset_of!(P, set_frame_ts),
        std::mem::offset_of!(P, frame_ts),
        size_of::<u32>(),
        size_of::<u32>(),
    ];
    for (i, value) in layout.into_iter().enumerate() {
        // SAFETY: plain value.
        assert_eq!(
            unsafe { oracle::oracle_source_deinterlace_select_layout(i) },
            value,
            "layout entry {i}"
        );
    }
}

const STEPS: [u64; 16] = [
    0,
    1,
    FRAME_TOLERANCE - 1,
    FRAME_TOLERANCE,
    FRAME_TOLERANCE + 1,
    S,
    16_666_667,
    MAX_TS_VAR - 1,
    MAX_TS_VAR,
    MAX_TS_VAR + 1,
    u64::MAX - S + 1,
    u64::MAX - FRAME_TOLERANCE,
    u64::MAX,
    1 << 63,
    (1 << 63) + 1,
    4 * MS,
];

/// Queues of 0 to 6 frames x buffered and unbuffered x last_frame_ts at,
/// around and far from the front frame, on every step, with the clock at
/// and around each frame and the tolerance and jump edges.
#[test]
fn walk_exhaustive_small_queues() {
    let mut runs = 0u64;
    let mut ready = 0u64;
    let mut flagged = 0u64;
    let mut resets = 0u64;
    let mut drops = 0u64;
    for n in 0..=6usize {
        for &start in &[T, 0, u64::MAX - 2 * S] {
            for &step in &STEPS {
                let ts: Vec<u64> = (0..n as u64)
                    .map(|i| start.wrapping_add(i.wrapping_mul(step)))
                    .collect();
                let front = ts.first().copied().unwrap_or(start);
                let lasts = [
                    0,
                    front,
                    front.wrapping_sub(10 * MS),
                    front.wrapping_add(3 * MS),
                    front.wrapping_sub(MAX_TS_VAR),
                    front.wrapping_sub(MAX_TS_VAR + 1),
                    front.wrapping_add(MAX_TS_VAR),
                    front.wrapping_add(MAX_TS_VAR + 1),
                    u64::MAX,
                ];
                for &last in &lasts {
                    for k in 0..=n as u64 {
                        for &d in &[
                            0u64,
                            FRAME_TOLERANCE - 1,
                            FRAME_TOLERANCE,
                            FRAME_TOLERANCE + 1,
                            5 * MS,
                            u64::MAX,
                        ] {
                            let clock = front.wrapping_add(k.wrapping_mul(step)).wrapping_add(d);
                            let sys_time = SYS.wrapping_add(clock.wrapping_sub(last));
                            for unbuffered in [false, true] {
                                let c = check_walk(&ts, sys_time, last, SYS, unbuffered);
                                runs += 1;
                                ready += u64::from(c.ready);
                                flagged += u64::from(c.flag_front);
                                resets += u64::from(c.offset_reset);
                                drops += u64::from(c.drop_front > 0);
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(runs > 140_000, "{runs}");
    for (what, count) in [
        ("ready", ready),
        ("flagged", flagged),
        ("resets", resets),
        ("drops", drops),
    ] {
        assert!(count > 1000, "{what}: {count}");
    }
}

/// Queues past MAX_ASYNC_FRAMES, every size from 7 to 40, on the steps.
#[test]
fn walk_long_queues() {
    for n in 7..=40usize {
        for &step in &STEPS {
            let ts: Vec<u64> = (0..n as u64)
                .map(|i| T.wrapping_add(i.wrapping_mul(step)))
                .collect();
            for k in [
                0,
                1,
                n as u64 / 2,
                n as u64 - 2,
                n as u64 - 1,
                n as u64,
                100,
            ] {
                for &d in &[0u64, FRAME_TOLERANCE, 5 * MS] {
                    let clock = T.wrapping_add(k.wrapping_mul(step)).wrapping_add(d);
                    for unbuffered in [false, true] {
                        check_walk(
                            &ts,
                            SYS.wrapping_add(clock.wrapping_sub(T)),
                            T,
                            SYS,
                            unbuffered,
                        );
                    }
                }
            }
        }
    }
}

const EDGES: [u64; 14] = [
    0,
    1,
    2,
    S,
    FRAME_TOLERANCE,
    MAX_TS_VAR,
    T,
    SYS,
    SYS - T,
    (1 << 63) - 1,
    1 << 63,
    u64::MAX - S,
    u64::MAX - 1,
    u64::MAX,
];

#[test]
fn small_decisions_exhaustive() {
    for &last in &EDGES {
        for count in [0usize, 1, 2, 3, 30, 33, usize::MAX] {
            check_first(last, count);
        }
    }
    for &a in &EDGES {
        for &b in &EDGES {
            for &o in &EDGES {
                for &t in &EDGES {
                    check_pair(a, b, o, t);
                }
            }
        }
    }
    for remaining in [0usize, 1, 2, usize::MAX] {
        // SAFETY: plain values.
        let c = unsafe { oracle::oracle_obs_source_deinterlace_select_take_reads_flag(remaining) };
        assert_eq!(
            u8::from(shim::obs_source_deinterlace_select_take_reads_flag(
                remaining
            )),
            c
        );
        assert_eq!(u8::from(core::take_reads_flag(remaining)), c);
        for flag in [false, true] {
            // SAFETY: plain values.
            let c =
                unsafe { oracle::oracle_obs_source_deinterlace_select_take_pair(remaining, flag) };
            assert_eq!(
                u8::from(shim::obs_source_deinterlace_select_take_pair(
                    remaining, flag
                )),
                c
            );
            assert_eq!(u8::from(core::take_pair(remaining, flag)), c);
        }
    }
    for &last in &EDGES {
        // SAFETY: plain value.
        let c = unsafe { oracle::oracle_obs_source_deinterlace_select_take_init_last(last) };
        assert_eq!(
            u8::from(shim::obs_source_deinterlace_select_take_init_last(last)),
            c
        );
        assert_eq!(u8::from(core::take_init_last(last)), c);
    }
    for b in [false, true] {
        // SAFETY: plain value.
        let c = unsafe { oracle::oracle_obs_source_deinterlace_select_take_offset(b) };
        assert_eq!(shim::obs_source_deinterlace_select_take_offset(b), c);
        assert_eq!(u32::from(core::take_offset(b)), c);
        for u in [false, true] {
            // SAFETY: plain values.
            let c = unsafe { oracle::oracle_obs_source_deinterlace_select_update(b, u) };
            assert_eq!(shim::obs_source_deinterlace_select_update(b, u), c);
            assert_eq!(u32::from(core::update(b, u)), c);
        }
    }
}

fn timestamps() -> impl Strategy<Value = Vec<u64>> {
    prop_oneof![
        prop::collection::vec(any::<u64>(), 0..40),
        prop::collection::vec(any::<u64>(), 0..40).prop_map(|mut v| {
            v.sort_unstable();
            v
        }),
        (any::<u64>(), any::<u64>(), 0..40usize).prop_map(|(start, step, n)| {
            (0..n as u64)
                .map(|i| start.wrapping_add(i.wrapping_mul(step)))
                .collect()
        }),
        (any::<u64>(), 0..3 * MAX_TS_VAR, 0..40usize).prop_map(|(start, step, n)| {
            (0..n as u64)
                .map(|i| start.wrapping_add(i.wrapping_mul(step)))
                .collect()
        }),
        (
            any::<u64>(),
            prop::collection::vec((0u8..8, 0..3 * MAX_TS_VAR), 0..40)
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
                            2 => t.wrapping_add(MAX_TS_VAR + d % 3).wrapping_sub(1),
                            3 => t.wrapping_add(FRAME_TOLERANCE + d % 3).wrapping_sub(1),
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

    /// Any timestamp vector, any clock, any state.
    #[test]
    fn walk_any(
        ts in timestamps(),
        last in any::<u64>(),
        near in any::<bool>(),
        delta in any::<u64>(),
        sys_time in any::<u64>(),
        sys_offset in 0..10 * S,
        near_sys in any::<bool>(),
        last_sys in any::<u64>(),
        unbuffered in any::<bool>(),
    ) {
        // half the time the clock near the front frame, so the walk runs
        let last = match (near, ts.first()) {
            (true, Some(&f)) => f.wrapping_sub(delta % (3 * S)),
            _ => last,
        };
        let sys_time = if near_sys { last_sys.wrapping_add(sys_offset) } else { sys_time };
        check_walk(&ts, sys_time, last, last_sys, unbuffered);
    }

    #[test]
    fn small_decisions_any(
        a in any::<u64>(), b in any::<u64>(), o in any::<u64>(), t in any::<u64>(),
        count in any::<usize>(),
    ) {
        check_first(a, count);
        check_pair(a, b, o, t);
        check_pair(a, a.wrapping_add(b % (3 * S)), o, a.wrapping_add(o).wrapping_add(t % (4 * S)));
    }
}

// ---------------------------------------------------------------------------
// Sequences through a model of the deinterlaced async_tick()

/// The decisions a model run goes through: the C or the shims.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    C,
    Shim,
}

fn first_of(side: Side, last: u64, count: usize) -> (bool, bool) {
    match side {
        Side::C => {
            // SAFETY: plain values.
            let r = unsafe { oracle::oracle_obs_source_deinterlace_select_first(last, count) };
            (r.first != 0, r.flag_front != 0)
        }
        Side::Shim => {
            let r = shim::obs_source_deinterlace_select_first(last, count);
            (r.first, r.flag_front)
        }
    }
}

fn walk_of(
    side: Side,
    ts: &[u64],
    sys_time: u64,
    last: u64,
    last_sys: u64,
    unbuffered: bool,
) -> WalkOut {
    match side {
        Side::C => walk_c(ts, sys_time, last, last_sys, unbuffered),
        Side::Shim => walk_shim(ts, sys_time, last, last_sys, unbuffered),
    }
}

fn pair_of(side: Side, ts: u64, next: u64, offset: u64, sys_time: u64) -> (bool, bool, u64) {
    match side {
        Side::C => {
            let (p, s, f) = pair_c(ts, next, offset, sys_time);
            (p != 0, s != 0, f)
        }
        Side::Shim => {
            let r = shim::obs_source_deinterlace_select_unbuffered_pair(ts, next, offset, sys_time);
            (r.prev_frame, r.set_frame_ts, r.frame_ts)
        }
    }
}

fn take_of(
    side: Side,
    remaining: usize,
    flag: bool,
    last: u64,
    unbuffered: bool,
) -> (bool, bool, bool, u32) {
    match side {
        // SAFETY: plain values.
        Side::C => unsafe {
            (
                oracle::oracle_obs_source_deinterlace_select_take_reads_flag(remaining) != 0,
                oracle::oracle_obs_source_deinterlace_select_take_pair(remaining, flag) != 0,
                oracle::oracle_obs_source_deinterlace_select_take_init_last(last) != 0,
                oracle::oracle_obs_source_deinterlace_select_take_offset(unbuffered),
            )
        },
        Side::Shim => (
            shim::obs_source_deinterlace_select_take_reads_flag(remaining),
            shim::obs_source_deinterlace_select_take_pair(remaining, flag),
            shim::obs_source_deinterlace_select_take_init_last(last),
            shim::obs_source_deinterlace_select_take_offset(unbuffered),
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Model {
    /// (id, timestamp, prev_frame) of each queued frame, oldest first
    queue: Vec<(u32, u64, bool)>,
    cur: Option<(u32, u64, bool)>,
    prev: Option<(u32, u64, bool)>,
    last_frame_ts: u64,
    last_sys_timestamp: u64,
    unbuffered: bool,
    offset: u64,
    frame_ts: u64,
    half: u32,
    steps: Vec<String>,
}

impl Model {
    fn new(unbuffered: bool) -> Self {
        Self {
            queue: Vec::new(),
            cur: None,
            prev: None,
            last_frame_ts: 0,
            last_sys_timestamp: 0,
            unbuffered,
            offset: 0,
            frame_ts: 0,
            half: 0,
            steps: Vec::new(),
        }
    }

    fn erase_front(&mut self) -> (u32, u64, bool) {
        let f = self.queue.remove(0);
        self.steps.push(format!("E{}", f.0));
        f
    }

    /// `async_tick()` with deinterlacing: `deinterlace_process_last_frame()`
    /// then `last_sys_timestamp`.
    fn tick(&mut self, side: Side, sys_time: u64, video_time: u64, interval: u64) {
        if let Some(p) = self.prev.take() {
            self.steps.push(format!("R{}", p.0));
        }
        if let Some(c) = self.cur.take() {
            self.steps.push(format!("R{}", c.0));
        }
        self.closest(side, sys_time, video_time, interval);
        self.last_sys_timestamp = sys_time;
    }

    fn closest(&mut self, side: Side, sys_time: u64, video_time: u64, interval: u64) {
        if self.unbuffered
            && self.offset != 0
            && deinterlace_pacing::hold(self.frame_ts, self.offset, self.half, sys_time)
        {
            self.steps.push("hold".into());
            return;
        }
        if self.queue.is_empty() {
            return;
        }
        let (first, flag_front) = first_of(side, self.last_frame_ts, self.queue.len());
        if flag_front {
            self.queue[0].2 = true;
            self.steps.push(format!("F{}=1", self.queue[0].0));
        }
        if !first && !self.ready(side, sys_time) {
            return;
        }
        let mut cur = self.erase_front();
        let remaining = self.queue.len();
        let reads = take_of(side, remaining, false, 0, false).0;
        let flag = reads && cur.2;
        let (_, pair, _, _) = take_of(side, remaining, flag, 0, false);
        if pair {
            self.prev = Some(cur);
            cur = self.erase_front();
            self.half = deinterlace_pacing::half_duration(
                cur.1,
                self.prev.map_or(0, |p| p.1),
                self.half,
                interval,
            );
        } else {
            self.half =
                deinterlace_pacing::half_duration(cur.1, self.frame_ts, self.half, interval);
        }
        self.cur = Some(cur);
        let init = take_of(side, 0, false, self.last_frame_ts, false).2;
        if init {
            self.last_frame_ts = cur.1;
        }
        self.frame_ts = cur.1;
        self.offset = if take_of(side, 0, false, 0, self.unbuffered).3 == 0 {
            deinterlace_pacing::buffered_offset(video_time, self.last_frame_ts)
        } else {
            deinterlace_pacing::reanchor(
                self.offset,
                video_time,
                self.frame_ts,
                self.half,
                interval,
            )
        };
    }

    fn ready(&mut self, side: Side, sys_time: u64) -> bool {
        let ts: Vec<u64> = self.queue.iter().map(|f| f.1).collect();
        let last = if self.unbuffered {
            0
        } else {
            self.last_frame_ts
        };
        let w = walk_of(
            side,
            &ts,
            sys_time,
            last,
            self.last_sys_timestamp,
            self.unbuffered,
        );
        for _ in 0..w.drop_front {
            let d = self.erase_front();
            self.steps.push(format!("R{}", d.0));
        }
        if w.pairs != 0 {
            let offset = if self.unbuffered { self.offset } else { 0 };
            let (prev_frame, set, frame_ts) = pair_of(
                side,
                ts[w.drop_front],
                ts[w.drop_front + 1],
                offset,
                sys_time,
            );
            if set {
                self.frame_ts = frame_ts;
            }
            self.queue[0].2 = prev_frame;
            self.steps
                .push(format!("F{}={}", self.queue[0].0, u8::from(prev_frame)));
        }
        self.last_frame_ts = w.last_frame_ts;
        if w.offset_reset != 0 {
            self.offset = 0;
        }
        if w.flag_front != 0 {
            self.queue[0].2 = true;
            self.steps.push(format!("F{}=1", self.queue[0].0));
        }
        w.ready != 0
    }
}

/// A small deterministic generator, so a failing sequence replays.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// Field rate (frames per second, numerator/denominator) into canvas rates,
/// buffered and unbuffered, clean and with jitter, bursts, stalls, jumps
/// and steps back. The steps and the model's state after every tick match.
#[test]
fn sequences_match() {
    let rates: [(u64, u64); 4] = [(25, 1), (30000, 1001), (50, 1), (60000, 1001)];
    let canvases: [(u64, u64); 3] = [(30, 1), (50, 1), (60000, 1001)];
    let mut ticks = 0u64;
    let mut seen = [0u64; 5];
    for (ri, &(fnum, fden)) in rates.iter().enumerate() {
        for (ci, &(cnum, cden)) in canvases.iter().enumerate() {
            for unbuffered in [false, true] {
                for chaos in 0..6u64 {
                    let seed = 0x9e37_79b9_7f4a_7c15
                        ^ ((ri as u64) << 32
                            | (ci as u64) << 16
                            | chaos << 8
                            | u64::from(unbuffered));
                    let mut rng = Rng(seed | 1);
                    let mut c = Model::new(unbuffered);
                    let mut s = Model::new(unbuffered);
                    let frame = 1_000_000_000 * fden / fnum;
                    let tick = 1_000_000_000 * cden / cnum;
                    let mut ts = 5_000_000_000u64;
                    let mut video = 900_000_000_000u64;
                    let mut id = 0u32;
                    let mut arrived = 0u64;
                    for n in 1..=600u64 {
                        // arrivals for this tick: clean, every frame whose
                        // time has come; else 0 to 3 at random
                        let due = (n * tick / frame).saturating_sub(arrived);
                        let arrivals = match (chaos, rng.next() % 16) {
                            (0, _) => due,
                            (_, 0) => 0,
                            (_, 1) => 3,
                            (_, 2..=4) => 2,
                            _ => 1,
                        };
                        for _ in 0..arrivals {
                            let mut t = ts;
                            if chaos > 0 {
                                match rng.next() % 64 {
                                    0 => {
                                        t = t
                                            .wrapping_add(MAX_TS_VAR + rng.next() % 3)
                                            .wrapping_sub(1)
                                    }
                                    1 => t = t.wrapping_sub(rng.next() % (4 * frame)),
                                    2 => t = rng.next(),
                                    3..=10 => {
                                        t = t.wrapping_add(rng.next() % (2 * MS)).wrapping_sub(MS)
                                    }
                                    _ => {}
                                }
                            }
                            for m in [&mut c, &mut s] {
                                m.queue.push((id, t, false));
                                if m.queue.len() > 30 {
                                    // cache_video()'s overflow: the cache is freed and the clock reset
                                    m.queue.clear();
                                    m.last_frame_ts = 0;
                                    m.steps.push("reset".into());
                                }
                            }
                            id += 1;
                            arrived += 1;
                            ts = t.wrapping_add(frame);
                        }
                        let step = if chaos > 0 && rng.next().is_multiple_of(50) {
                            tick * (rng.next() % 5)
                        } else {
                            tick
                        };
                        video = video.wrapping_add(step);
                        if chaos > 0 && rng.next().is_multiple_of(200) {
                            video = video.wrapping_sub(3 * tick);
                        }
                        c.tick(Side::C, video, video, tick);
                        s.tick(Side::Shim, video, video, tick);
                        assert_eq!(s, c, "seed {seed:#x} tick {ticks}");
                        ticks += 1;
                    }
                    for step in &c.steps {
                        let kind = match step.as_str() {
                            "hold" => 0,
                            "reset" => 1,
                            x if x.ends_with("=0") => 2,
                            x if x.ends_with("=1") => 3,
                            x if x.starts_with('R') => 4,
                            _ => continue,
                        };
                        seen[kind] += 1;
                    }
                }
            }
        }
    }
    assert!(ticks > 80_000, "{ticks}");
    // every kind of step: holds, cache resets, waits, pairs, releases
    assert!(seen.iter().all(|&n| n > 100), "{seen:?}");
}
