//! Tier 3: the `obs_source_async_pacing` shim against the original C
//! (`obs-c-oracle`, `oracle/source_async_pacing.c`: the unmodified
//! `libobs/obs-source-async-pacing.c.h` with its function renamed).
//!
//! The function takes the queue's timestamps and plain integers and returns
//! `struct obs_source_async_pacing_result`, so a run compares the bytes of
//! the two results: `drop_front` and `last_frame_ts` as their
//! little-endian bytes, and `take_first` as the byte the C stores.
//!
//! Runs:
//! - the two thresholds and the result's layout against the C header;
//! - every queue of up to three of the special values of the full `u64`
//!   range and the thresholds +-1, from each special `last_frame_ts`, over
//!   canvas steps at the guard and jump edges, backwards and wrapping, both
//!   bufferings;
//! - proptests over full-range `u64` queues of 1 to 30 frames, and over
//!   queues near the clock with the guard and jump edges as likely as
//!   anywhere;
//! - sequences through a model of `async_tick()`, `get_closest_frame()` and
//!   `cache_video()` (the queue, its 30-frame limit and reset, the erases
//!   from the front, the store of `last_frame_ts` and of
//!   `last_sys_timestamp`, the frame taken), one copy driven by the C and
//!   one by the shim: sources of 23.976, 25, 29.97, 30, 50 and 60 frames a
//!   second into 30 and 60 fps canvases, buffered and unbuffered, with
//!   timestamp and arrival jitter, late deliveries, bursts and stalls, late
//!   ticks, ticks that stop (the queue fills and resets), a canvas clock
//!   going back, timestamps that jump ahead or back
//!   by any amount or land anywhere, queue resets
//!   (`obs_source_output_video(NULL)`) and the buffering toggled. The bytes
//!   of every call's result and the model's state after every tick must be
//!   equal. Every rate pair, buffered and unbuffered, also runs clean and
//!   jittered from fixed seeds, and a test checks that the random
//!   sequences reach every rule more than 100 times;
//! - clean buffered sequences also show their frames in order, never twice.
//!
//! Intentional difference (not compared): a null `timestamps` with a count
//! above 0 is an empty queue for the shim; the C reads through it.

use std::collections::VecDeque;

use obs_c_oracle::source_async_pacing as oracle;
use obs_source::async_pacing::{FRAME_TOLERANCE, MAX_TS_VAR, State, frame_out_of_bounds};
use obs_source::ffi::source_async_pacing as shim;
use proptest::prelude::*;
use proptest::strategy::ValueTree;

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
/// `MAX_ASYNC_FRAMES` (`obs-source.c.h`).
const MAX_ASYNC_FRAMES: usize = 30;

/// A result as the C lays its fields out, `take_first` as its byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Out {
    drop_front: usize,
    last_frame_ts: u64,
    take_first: u8,
}

impl Out {
    fn bytes(self) -> Vec<u8> {
        let mut v = Vec::with_capacity(17);
        v.extend_from_slice(&self.drop_front.to_le_bytes());
        v.extend_from_slice(&self.last_frame_ts.to_le_bytes());
        v.push(self.take_first);
        v
    }
}

trait Pacer {
    fn pace(&self, ts: &[u64], sys_time: u64, state: State) -> Out;
}

struct C;
struct Rust;

fn ptr(ts: &[u64]) -> *const u64 {
    if ts.is_empty() {
        std::ptr::null()
    } else {
        ts.as_ptr()
    }
}

impl Pacer for C {
    fn pace(&self, ts: &[u64], sys_time: u64, state: State) -> Out {
        // SAFETY: ts.len() readable timestamps, or NULL with a count of 0.
        let r = unsafe {
            oracle::oracle_obs_source_async_pacing(
                ptr(ts),
                ts.len(),
                sys_time,
                state.last_frame_ts,
                state.last_sys_timestamp,
                state.async_unbuffered,
            )
        };
        Out {
            drop_front: r.drop_front,
            last_frame_ts: r.last_frame_ts,
            take_first: r.take_first,
        }
    }
}

impl Pacer for Rust {
    fn pace(&self, ts: &[u64], sys_time: u64, state: State) -> Out {
        // SAFETY: as for the C.
        let r = unsafe {
            shim::obs_source_async_pacing(
                ptr(ts),
                ts.len(),
                sys_time,
                state.last_frame_ts,
                state.last_sys_timestamp,
                state.async_unbuffered,
            )
        };
        Out {
            drop_front: r.drop_front,
            last_frame_ts: r.last_frame_ts,
            take_first: u8::from(r.take_first),
        }
    }
}

#[track_caller]
fn check(ts: &[u64], sys_time: u64, state: State) {
    let c = C.pace(ts, sys_time, state);
    let r = Rust.pace(ts, sys_time, state);
    assert_eq!(
        c.bytes(),
        r.bytes(),
        "pace({ts:?}, {sys_time}, {state:?}): C {c:?}, Rust {r:?}"
    );
}

#[test]
fn constants_and_layout_match_the_c_header() {
    let ours = [MAX_TS_VAR, FRAME_TOLERANCE];
    for (i, name) in oracle::CONSTANT_NAMES.iter().enumerate() {
        // SAFETY: an index in range.
        let c = unsafe { oracle::oracle_source_async_pacing_constant(i) };
        assert_eq!(c, ours[i], "{name}");
    }
    // SAFETY: indexes in range.
    let layout: Vec<usize> = (0..4)
        .map(|i| unsafe { oracle::oracle_source_async_pacing_result_layout(i) })
        .collect();
    assert_eq!(
        layout,
        [
            std::mem::size_of::<shim::obs_source_async_pacing_result>(),
            std::mem::offset_of!(shim::obs_source_async_pacing_result, drop_front),
            std::mem::offset_of!(shim::obs_source_async_pacing_result, last_frame_ts),
            std::mem::offset_of!(shim::obs_source_async_pacing_result, take_first),
        ]
    );
}

/// A source clock away from both ends, and a canvas clock on another base.
const L: u64 = 100 * SEC;
const VT: u64 = 5000 * SEC;

const SPECIAL: [u64; 20] = [
    0,
    1,
    FRAME_TOLERANCE - 1,
    FRAME_TOLERANCE,
    MAX_TS_VAR,
    MAX_TS_VAR + 1,
    L - MAX_TS_VAR - 1,
    L - MAX_TS_VAR,
    L - FRAME_TOLERANCE - 1,
    L - FRAME_TOLERANCE,
    L - 1,
    L,
    L + 1,
    L + FRAME_TOLERANCE,
    L + MAX_TS_VAR,
    L + MAX_TS_VAR + 1,
    (1 << 63) - 1,
    1 << 63,
    u64::MAX - MAX_TS_VAR,
    u64::MAX,
];

/// (last_sys_timestamp, sys_time): no time, a tick at 60 fps, the guard and
/// jump edges, backwards, wrapping.
const CLOCKS: [(u64, u64); 10] = [
    (VT, VT),
    (VT, VT + 16_666_666),
    (VT, VT + FRAME_TOLERANCE - 1),
    (VT, VT + FRAME_TOLERANCE),
    (VT, VT + MAX_TS_VAR),
    (VT, VT + MAX_TS_VAR + 1),
    (VT, VT - 1),
    (VT, VT - 100 * MS),
    (VT, 0),
    (1, 0),
];

#[test]
fn special_values() {
    for lft in SPECIAL {
        for (lst, sys) in CLOCKS {
            for unbuffered in [false, true] {
                let state = State {
                    last_frame_ts: lft,
                    last_sys_timestamp: lst,
                    async_unbuffered: unbuffered,
                };
                check(&[], sys, state);
                for a in SPECIAL {
                    check(&[a], sys, state);
                    for b in SPECIAL {
                        check(&[a, b], sys, state);
                    }
                }
            }
        }
    }
}

#[test]
fn special_values_of_three() {
    for lft in [0, 1, L, 1 << 63, u64::MAX] {
        for (lst, sys) in CLOCKS {
            let state = State {
                last_frame_ts: lft,
                last_sys_timestamp: lst,
                async_unbuffered: false,
            };
            for a in SPECIAL {
                for b in SPECIAL {
                    for c in SPECIAL {
                        check(&[a, b, c], sys, state);
                    }
                }
            }
        }
    }
}

/// A value near one of `points` (within +-4) as often as anywhere.
fn near_or_any(points: &'static [u64]) -> impl Strategy<Value = u64> {
    prop_oneof![
        any::<u64>(),
        (prop::sample::select(points), -4i64..=4).prop_map(|(p, d)| p.wrapping_add_signed(d)),
    ]
}

const POINTS: [u64; 8] = [
    0,
    L,
    L - FRAME_TOLERANCE,
    L + MAX_TS_VAR,
    L - MAX_TS_VAR,
    VT,
    1 << 63,
    u64::MAX,
];

fn state_strategy() -> impl Strategy<Value = State> {
    (
        prop_oneof![Just(0u64), near_or_any(&POINTS)],
        near_or_any(&POINTS),
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
    #![proptest_config(ProptestConfig::with_cases(20000))]

    #[test]
    fn full_range_queues(
        ts in prop::collection::vec(any::<u64>(), 1..=30),
        sys_time in any::<u64>(),
        state in state_strategy(),
    ) {
        check(&ts, sys_time, state);
    }

    /// Queues near the clock: each step from the last frame drawn near the
    /// guard, the jump threshold, a frame interval, 0 or backwards.
    #[test]
    fn queues_near_the_clock(
        start in near_or_any(&POINTS),
        steps in prop::collection::vec(
            prop_oneof![
                (FRAME_TOLERANCE - 2)..(FRAME_TOLERANCE + 2),
                (MAX_TS_VAR - 2)..(MAX_TS_VAR + 2),
                (10 * MS)..(45 * MS),
                Just(0u64),
                any::<u64>(),
            ],
            0..30,
        ),
        lft_delta in prop_oneof![
            -(4 * MS as i64)..(4 * MS as i64),
            -(MAX_TS_VAR as i64 + 2)..-(MAX_TS_VAR as i64 - 2),
            (MAX_TS_VAR as i64 - 2)..(MAX_TS_VAR as i64 + 2),
        ],
        advance in prop_oneof![0u64..(50 * MS), any::<u64>()],
        unbuffered in any::<bool>(),
    ) {
        let mut ts = vec![start];
        for s in steps {
            let last = ts[ts.len() - 1];
            ts.push(last.wrapping_add(s));
        }
        let state = State {
            last_frame_ts: start.wrapping_add_signed(lft_delta),
            last_sys_timestamp: VT,
            async_unbuffered: unbuffered,
        };
        check(&ts, VT.wrapping_add(advance), state);
    }
}

// --- sequences ---------------------------------------------------------------

/// A source's async state: `async_frames` (timestamps), `last_frame_ts`,
/// `last_sys_timestamp`, `async_unbuffered`, and the frame the last tick
/// took (`cur_async_frame`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Source {
    queue: VecDeque<u64>,
    last_frame_ts: u64,
    last_sys_timestamp: u64,
    unbuffered: bool,
    shown: Option<u64>,
}

/// How often the rules were reached, from the C's inputs and results.
#[derive(Debug, Default)]
struct Reached {
    first_frame: usize,
    unbuffered: usize,
    jump: usize,
    walk_nothing: usize,
    walk_front: usize,
    walk_dropped: usize,
    walk_to_last: usize,
    walk_guard: usize,
    walk_reanchor_or_seed: usize,
    full_queue: usize,
}

impl Reached {
    fn record(&mut self, ts: &[u64], sys_time: u64, state: State, out: Out) {
        if state.last_frame_ts == 0 {
            self.first_frame += 1;
            return;
        }
        if state.async_unbuffered {
            self.unbuffered += 1;
            return;
        }
        if frame_out_of_bounds(state.last_frame_ts, ts[0]) {
            self.jump += 1;
            return;
        }
        let advanced = state
            .last_frame_ts
            .wrapping_add(sys_time.wrapping_sub(state.last_sys_timestamp));
        if out.take_first == 0 {
            self.walk_nothing += 1;
            return;
        }
        if out.last_frame_ts != advanced {
            self.walk_reanchor_or_seed += 1;
        }
        match out.drop_front {
            d if d + 1 == ts.len() && d > 0 => self.walk_to_last += 1,
            0 => self.walk_front += 1,
            _ => self.walk_dropped += 1,
        }
        if let Some(&next) = ts.get(out.drop_front + 1)
            && out.last_frame_ts > next
        {
            self.walk_guard += 1;
        }
    }

    fn all_above(&self, n: usize) -> bool {
        [
            self.first_frame,
            self.unbuffered,
            self.jump,
            self.walk_nothing,
            self.walk_front,
            self.walk_dropped,
            self.walk_to_last,
            self.walk_guard,
            self.walk_reanchor_or_seed,
            self.full_queue,
        ]
        .iter()
        .all(|&c| c > n)
    }
}

impl Source {
    /// `cache_video()` and the queue append of
    /// `obs_source_output_video_internal()`.
    fn output(&mut self, ts: u64, reached: &mut Reached) {
        if self.queue.len() >= MAX_ASYNC_FRAMES {
            // free_async_cache(), last_frame_ts = 0, the frame not queued
            self.queue.clear();
            self.last_frame_ts = 0;
            reached.full_queue += 1;
            return;
        }
        self.queue.push_back(ts);
    }

    /// `obs_source_output_video(source, NULL)`.
    fn reset(&mut self) {
        self.queue.clear();
        self.last_frame_ts = 0;
    }

    /// `async_tick()` without deinterlacing: `get_closest_frame()` and the
    /// store of `last_sys_timestamp`.
    fn tick<P: Pacer>(&mut self, p: &P, sys_time: u64, log: &mut Vec<u8>, reached: &mut Reached) {
        self.shown = None;
        if !self.queue.is_empty() {
            let ts: Vec<u64> = self.queue.iter().copied().collect();
            let state = State {
                last_frame_ts: self.last_frame_ts,
                last_sys_timestamp: self.last_sys_timestamp,
                async_unbuffered: self.unbuffered,
            };
            let out = p.pace(&ts, sys_time, state);
            log.extend(out.bytes());
            reached.record(&ts, sys_time, state, out);
            assert!(out.drop_front < ts.len(), "{out:?} for {ts:?}");
            for _ in 0..out.drop_front {
                self.queue.pop_front();
            }
            self.last_frame_ts = out.last_frame_ts;
            if out.take_first != 0 {
                self.shown = self.queue.pop_front();
            }
        }
        self.last_sys_timestamp = sys_time;
    }
}

#[derive(Debug, Clone)]
enum Event {
    /// The source clock jumps ahead by this much.
    JumpAhead(u64),
    /// The source clock goes back by this much.
    JumpBack(u64),
    /// The source clock lands anywhere.
    Anywhere(u64),
    /// `obs_source_output_video(NULL)`.
    Reset,
    ToggleBuffering,
    /// This tick comes late by this much.
    LateTick(u64),
    /// The canvas clock goes back by this much at this tick.
    CanvasBack(u64),
    /// The next n frames are delivered late, all at once.
    Burst(u64),
    /// The next n frames are lost.
    Stall(u64),
    /// The next n ticks do not happen (a source that is not shown keeps
    /// outputting), so the queue can fill.
    NoTicks(u64),
}

#[derive(Debug, Clone)]
struct Sequence {
    source_rate: (u64, u64),
    canvas_rate: (u64, u64),
    unbuffered: bool,
    /// The source clock less the canvas clock at capture.
    clock_offset: u64,
    canvas_base: u64,
    latency: u64,
    ts_jitter: u64,
    arrival_jitter: u64,
    seed: u64,
    ticks: u64,
    events: Vec<(u64, Event)>,
}

const SOURCE_RATES: [(u64, u64); 6] = [
    (24000, 1001),
    (25, 1),
    (30000, 1001),
    (30, 1),
    (50, 1),
    (60, 1),
];
const CANVAS_RATES: [(u64, u64); 2] = [(30, 1), (60, 1)];

/// The time of frame or tick n at num/den a second, in ns.
fn rate_time(n: u64, (num, den): (u64, u64)) -> u64 {
    let ns = u128::from(n) * u128::from(SEC) * u128::from(den) / u128::from(num);
    u64::try_from(ns).unwrap_or(u64::MAX)
}

fn mix(seed: u64, n: u64) -> u64 {
    let mut z = seed ^ n.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

fn jitter(seed: u64, n: u64, amount: u64) -> u64 {
    if amount == 0 {
        0
    } else {
        mix(seed, n) % amount
    }
}

/// The run of a sequence: the bytes of every call's result, the state after
/// every tick, and what was reached.
fn run<P: Pacer>(p: &P, seq: &Sequence) -> (Vec<u8>, Vec<Source>, Reached) {
    let mut source = Source {
        unbuffered: seq.unbuffered,
        last_sys_timestamp: seq.canvas_base,
        ..Source::default()
    };
    let mut log = Vec::new();
    let mut states = Vec::new();
    let mut reached = Reached::default();
    let mut offset = seq.clock_offset;
    let mut next_frame = 0u64;
    // frames below burst_until arrive together, those below lost_until never
    let mut burst_until = 0u64;
    let mut lost_until = 0u64;
    let mut canvas_back = 0u64;
    let mut no_ticks_until = 0u64;

    for k in 0..seq.ticks {
        let mut late = 0;
        for (at, event) in &seq.events {
            if *at != k {
                continue;
            }
            match event {
                Event::JumpAhead(d) => offset = offset.wrapping_add(*d),
                Event::JumpBack(d) => offset = offset.wrapping_sub(*d),
                Event::Anywhere(v) => offset = *v,
                Event::Reset => source.reset(),
                Event::ToggleBuffering => source.unbuffered = !source.unbuffered,
                Event::LateTick(d) => late = *d,
                Event::CanvasBack(d) => canvas_back = canvas_back.wrapping_add(*d),
                Event::Burst(n) => burst_until = next_frame + n,
                Event::Stall(n) => lost_until = next_frame + n,
                Event::NoTicks(n) => no_ticks_until = k + n,
            }
        }
        // canvas time since the base, then the tick
        let rel_tick = rate_time(k, seq.canvas_rate).wrapping_add(late);
        // deliver every frame that has arrived by this tick
        loop {
            let cap = rate_time(next_frame, seq.source_rate);
            // a burst delivers its frames together at the burst's end
            let arrival = if next_frame < burst_until {
                rate_time(burst_until, seq.source_rate) + seq.latency
            } else {
                cap + seq.latency + jitter(seq.seed, next_frame, seq.arrival_jitter)
            };
            if arrival > rel_tick {
                break;
            }
            if next_frame >= lost_until {
                let ts = cap.wrapping_add(offset).wrapping_add(jitter(
                    seq.seed ^ 1,
                    next_frame,
                    seq.ts_jitter,
                ));
                source.output(ts, &mut reached);
            }
            next_frame += 1;
        }
        let sys_time = seq
            .canvas_base
            .wrapping_add(rel_tick)
            .wrapping_sub(canvas_back);
        if k >= no_ticks_until {
            source.tick(p, sys_time, &mut log, &mut reached);
        }
        states.push(source.clone());
    }
    (log, states, reached)
}

fn event() -> impl Strategy<Value = Event> {
    prop_oneof![
        prop_oneof![
            Just(MAX_TS_VAR),
            Just(MAX_TS_VAR + 1),
            (10 * MS)..(5 * SEC),
            any::<u64>()
        ]
        .prop_map(Event::JumpAhead),
        prop_oneof![
            Just(MAX_TS_VAR),
            Just(MAX_TS_VAR + 1),
            1..(5 * SEC),
            any::<u64>()
        ]
        .prop_map(Event::JumpBack),
        any::<u64>().prop_map(Event::Anywhere),
        Just(Event::Reset),
        Just(Event::ToggleBuffering),
        (MS..(80 * MS)).prop_map(Event::LateTick),
        (1..(40 * MS)).prop_map(Event::CanvasBack),
        (2u64..40).prop_map(Event::Burst),
        (1u64..20).prop_map(Event::Stall),
        (1u64..80).prop_map(Event::NoTicks),
    ]
}

fn sequence() -> impl Strategy<Value = Sequence> {
    (
        prop::sample::select(SOURCE_RATES.to_vec()),
        prop::sample::select(CANVAS_RATES.to_vec()),
        any::<bool>(),
        prop_oneof![Just(0u64), any::<u64>(), (SEC..(1000 * SEC))],
        prop_oneof![Just(0u64), any::<u64>(), (SEC..(1000 * SEC))],
        0u64..(60 * MS),
        prop_oneof![Just(0u64), 1..(4 * MS)],
        prop_oneof![Just(0u64), 1..(20 * MS)],
        any::<u64>(),
        2u64..300,
        prop::collection::vec((0u64..300, event()), 0..8),
    )
        .prop_map(
            |(
                source_rate,
                canvas_rate,
                unbuffered,
                clock_offset,
                canvas_base,
                latency,
                ts_jitter,
                arrival_jitter,
                seed,
                ticks,
                events,
            )| Sequence {
                source_rate,
                canvas_rate,
                unbuffered,
                clock_offset,
                canvas_base,
                latency,
                ts_jitter,
                arrival_jitter,
                seed,
                ticks,
                events,
            },
        )
}

#[track_caller]
fn check_sequence(seq: &Sequence) -> Reached {
    let (c_log, c_states, reached) = run(&C, seq);
    let (r_log, r_states, _) = run(&Rust, seq);
    for (k, (c, r)) in c_states.iter().zip(&r_states).enumerate() {
        assert_eq!(c, r, "state after tick {k} of {seq:?}");
    }
    assert_eq!(c_log, r_log, "results of {seq:?}");
    reached
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(3000))]

    #[test]
    fn sequences(seq in sequence()) {
        check_sequence(&seq);
    }
}

/// Every rate pair, buffered and unbuffered, clean and jittered, from fixed
/// seeds; the clean buffered runs show their frames in order and never
/// twice.
#[test]
fn rate_pairs() {
    for source_rate in SOURCE_RATES {
        for canvas_rate in CANVAS_RATES {
            for unbuffered in [false, true] {
                for (i, (ts_jitter, arrival_jitter, latency)) in
                    [(0, 0, 5 * MS), (3 * MS, 15 * MS, 20 * MS)]
                        .into_iter()
                        .enumerate()
                {
                    let seq = Sequence {
                        source_rate,
                        canvas_rate,
                        unbuffered,
                        clock_offset: 100 * SEC,
                        canvas_base: 5000 * SEC,
                        latency,
                        ts_jitter,
                        arrival_jitter,
                        seed: 0x5eed ^ (i as u64),
                        ticks: 600,
                        events: vec![
                            (200, Event::JumpAhead(MAX_TS_VAR + 1)),
                            (400, Event::JumpBack(3 * SEC)),
                        ],
                    };
                    check_sequence(&seq);
                    if i == 0 && !unbuffered {
                        let clean = Sequence {
                            events: Vec::new(),
                            ..seq.clone()
                        };
                        check_sequence(&clean);
                        let (_, states, _) = run(&Rust, &clean);
                        let shown: Vec<u64> = states.iter().filter_map(|s| s.shown).collect();
                        assert!(
                            shown.windows(2).all(|w| w[0] < w[1]),
                            "{source_rate:?} into {canvas_rate:?}: out of order or twice"
                        );
                        assert!(shown.len() > 100, "{source_rate:?} into {canvas_rate:?}");
                    }
                }
            }
        }
    }
}

/// The random sequences reach every rule more than 100 times.
#[test]
fn sequences_reach_every_rule() {
    let mut runner = proptest::test_runner::TestRunner::deterministic();
    let strategy = sequence();
    let mut total = Reached::default();
    for _ in 0..2000 {
        let seq = strategy
            .new_tree(&mut runner)
            .expect("a sequence")
            .current();
        let r = check_sequence(&seq);
        total.first_frame += r.first_frame;
        total.unbuffered += r.unbuffered;
        total.jump += r.jump;
        total.walk_nothing += r.walk_nothing;
        total.walk_front += r.walk_front;
        total.walk_dropped += r.walk_dropped;
        total.walk_to_last += r.walk_to_last;
        total.walk_guard += r.walk_guard;
        total.walk_reanchor_or_seed += r.walk_reanchor_or_seed;
        total.full_queue += r.full_queue;
    }
    assert!(total.all_above(100), "{total:?}");
}
