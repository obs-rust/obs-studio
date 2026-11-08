//! Tier 3 for `obs_source_seam::ffi::source_transition_stop` against the
//! original `libobs/obs-source-transition-stop.c.h`
//! (`obs_c_oracle::source_transition_stop`, renamed `oracle_*`).
//!
//! Both sides run on the same graph, built afresh for each: a test
//! transition, a source that is not a transition and two children (zeroed
//! `struct obs_source` with a type, a name and both transition mutexes),
//! each with a reference count, and an active-child link count per parent
//! and child. The C calls the oracle's recording stand-ins for
//! `obs_source_release()`, `obs_source_remove_active_child()`,
//! `obs_source_dosignal()` and the video frame time, and the type's
//! `transition_stop` is a recording callback; the Rust shims are run with
//! [`Oracle`], whose `Calls` are those same stand-ins, and reach the fields,
//! the lock and the callback through the C transition seam, as in the
//! hybrid libobs. The frame time and the callback record whether the
//! transition's lock was held.
//!
//! After every call the two must agree on what it returned (an `f32`, bit
//! for bit), the transition's slots and active flags, its mode, manual value
//! and clock, which mutexes of each source are held, every reference count
//! and link count, and every call in order. The helpers the renders call
//! with the lock held (`transition_calc_time()`,
//! `transition_get_video_time()` and `transition_stop()`) run under it, as
//! there. The inputs are every call on fixed graphs (each slot empty, or
//! holding a child, active or not; a type with and without a callback;
//! clocks before, during and after a start, in each mode), and a proptest
//! over random graphs (any mode, including ones outside the enum, any
//! manual value, start time, duration and frame time, the same child in
//! both slots) and call sequences.
//!
//! Intentional difference from C: `obs_transition_get_time()` and
//! `obs_transition_force_stop()` check their argument with
//! `transition_valid()`. On NULL the C crashes, so only the Rust side runs:
//! it returns 0 and calls nothing. On a source that is not a transition the
//! C reads that source's unused fields (its time is 1 here) and calls its
//! `transition_stop` and signals the stop; the Rust side returns 0 and
//! calls nothing, both checked against the C's outcome.

use std::ffi::{c_char, c_int, c_void};
use std::ptr;

use obs_c_oracle::parity::cases;
use obs_c_oracle::source_transition_slots::{self as slots, graph_lock, seam};
use obs_c_oracle::source_transition_stop as c;
use obs_c_oracle::source_transition_switch as sw;
use obs_source_seam::ffi::source_transition_stop::{self as rs, Calls};
use proptest::prelude::*;

/// `OBS_SOURCE_TYPE_INPUT`.
const INPUT: c_int = 0;
/// `OBS_SOURCE_TYPE_TRANSITION`.
const TRANSITION: c_int = 2;

/// `OBS_TRANSITION_MODE_AUTO`, `OBS_TRANSITION_MODE_MANUAL`.
const AUTO: c_int = 0;
const MANUAL: c_int = 1;

/// The sources of a graph, in the order they are made.
const TR: usize = 0;
const NOT_TR: usize = 1;
const CH_A: usize = 2;
const CH_B: usize = 3;
const SOURCES: usize = 4;

const NAMES: [&std::ffi::CStr; SOURCES] = [c"stop transition", c"an input", c"child a", c"child b"];

/// The oracle's stand-ins, as the Rust shims' libobs calls.
enum Oracle {}

// SAFETY: each is the stand-in the C oracle calls under the same name.
unsafe impl Calls for Oracle {
    unsafe fn video_time() -> u64 {
        // SAFETY: see the trait.
        unsafe { c::oracle_stop_video_time() }
    }
    unsafe fn dosignal(
        transition: *mut c_void,
        signal_obs: *const c_char,
        signal_source: *const c_char,
    ) {
        // SAFETY: see the trait.
        unsafe { sw::oracle_switch_dosignal(transition, signal_obs, signal_source) }
    }
    unsafe fn remove_active_child(parent: *mut c_void, child: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { slots::oracle_slots_remove_active_child(parent, child) }
    }
    unsafe fn release(source: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { slots::oracle_slots_release(source) }
    }
}

/// One slot of the transition: which source it holds and whether it is
/// active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Slot {
    source: Option<usize>,
    active: bool,
}

/// How a graph starts.
#[derive(Debug, Clone)]
struct Init {
    slots: [Slot; 2],
    mode: c_int,
    manual_val: f32,
    start_time: u64,
    duration: u64,
    video_time: u64,
    /// The type has a `transition_stop` (the input gets one too).
    plugin: bool,
}

/// The source a public call is handed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Null,
    Tr,
    NotTr,
}

/// One call.
#[derive(Debug, Clone, Copy)]
enum Op {
    GetTime(Target),
    ForceStop(Target),
    /// `transition_calc_time(tr, ts)`, under the lock.
    CalcTime(u64),
    /// `transition_get_video_time(tr)`, under the lock.
    GetVideoTime,
    /// `transition_stop(tr)`, under the lock.
    Stop,
    HandleStop,
}

impl Op {
    /// Whether the C and Rust differ on purpose for this call.
    fn checks_argument(self) -> Option<Target> {
        match self {
            Self::GetTime(t) | Self::ForceStop(t) if t != Target::Tr => Some(t),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    C,
    Rust,
}

/// A graph's sources.
struct Graph {
    src: [*mut c_void; SOURCES],
}

impl Graph {
    /// Builds `init` on a fresh oracle graph.
    fn new(init: &Init) -> Self {
        let mut src = [ptr::null_mut(); SOURCES];
        // SAFETY: the oracle's constructors and the seam setters on its
        // live sources; the caller holds the graph lock.
        unsafe {
            slots::oracle_slots_reset();
            sw::oracle_switch_reset();
            for (i, s) in src.iter_mut().enumerate() {
                let kind = if i == TR { TRANSITION } else { INPUT };
                *s = slots::oracle_slots_source_new(kind, NAMES[i].as_ptr());
                assert!(!s.is_null());
            }
            let tr = src[TR];
            for (idx, slot) in init.slots.iter().enumerate() {
                let Some(child) = slot.source else { continue };
                seam::obs_source_transition_seam_set_sources(tr, idx, src[child]);
                seam::obs_source_transition_seam_set_source_active(tr, idx, slot.active);
                let refs = slots::oracle_slots_refs(src[child]);
                slots::oracle_slots_set_refs(src[child], refs + 1);
                if slot.active {
                    let links = slots::oracle_slots_link(tr, src[child]);
                    slots::oracle_slots_set_link(tr, src[child], links + 1);
                }
            }
            seam::obs_source_transition_seam_set_mode(tr, init.mode);
            seam::obs_source_transition_seam_set_manual_val(tr, init.manual_val);
            seam::obs_source_transition_seam_set_start_time(tr, init.start_time);
            seam::obs_source_transition_seam_set_duration(tr, init.duration);
            if init.plugin {
                c::oracle_stop_plugin(tr);
                c::oracle_stop_plugin(src[NOT_TR]);
            }
            c::oracle_stop_reset(tr, init.video_time);
        }
        Self { src }
    }

    fn id(&self, p: *mut c_void) -> Option<usize> {
        (!p.is_null()).then(|| self.src.iter().position(|s| *s == p).unwrap_or(99))
    }

    fn target(&self, t: Target) -> *mut c_void {
        match t {
            Target::Null => ptr::null_mut(),
            Target::Tr => self.src[TR],
            Target::NotTr => self.src[NOT_TR],
        }
    }
}

/// What a graph looks like after a call.
#[derive(Debug, Clone, PartialEq)]
struct Snapshot {
    /// The `f32` a call returned, as its bits.
    result: Option<u32>,
    slots: [(Option<usize>, bool); 2],
    clock: (c_int, u32, u64, u64),
    held: [c_int; SOURCES],
    refs: [i64; SOURCES],
    links: [i64; SOURCES],
    /// (kind, a, b, result) of every traced call.
    trace: Vec<(c_int, Option<usize>, Option<usize>, c_int)>,
}

fn snapshot(g: &Graph, result: Option<f32>) -> Snapshot {
    let tr = g.src[TR];
    // SAFETY: the oracle's getters on its live sources.
    unsafe {
        let slot = |idx| {
            (
                g.id(seam::obs_source_transition_seam_get_sources(tr, idx)),
                seam::obs_source_transition_seam_get_source_active(tr, idx),
            )
        };
        let mut trace = Vec::new();
        for i in 0..slots::oracle_slots_traced() {
            let (mut kind, mut a, mut b, mut r) = (0, ptr::null_mut(), ptr::null_mut(), 0);
            slots::oracle_slots_event(i, &raw mut kind, &raw mut a, &raw mut b, &raw mut r);
            trace.push((kind, g.id(a), g.id(b), r));
        }
        Snapshot {
            result: result.map(f32::to_bits),
            slots: [slot(0), slot(1)],
            clock: (
                seam::obs_source_transition_seam_get_mode(tr),
                seam::obs_source_transition_seam_get_manual_val(tr).to_bits(),
                seam::obs_source_transition_seam_get_start_time(tr),
                seam::obs_source_transition_seam_get_duration(tr),
            ),
            held: g.src.map(|s| c::oracle_stop_held(s)),
            refs: g.src.map(|s| slots::oracle_slots_refs(s)),
            links: g.src.map(|s| slots::oracle_slots_link(tr, s)),
            trace,
        }
    }
}

/// Runs `op` on `g` through `side`; the helpers under the lock, as the
/// renders call them.
fn run(g: &Graph, side: Side, op: Op) -> Option<f32> {
    let tr = g.src[TR];
    // SAFETY: the graph's live sources (or NULL where a public call takes
    // it, which the C side never gets), the lock held where the C's callers
    // hold it.
    unsafe {
        let locked = |f: &dyn Fn() -> Option<f32>| {
            assert_eq!(c::oracle_stop_lock(tr), 0);
            let r = f();
            assert_eq!(c::oracle_stop_unlock(tr), 0);
            r
        };
        match (side, op) {
            (Side::C, Op::GetTime(t)) => Some(c::oracle_obs_transition_get_time(g.target(t))),
            (Side::Rust, Op::GetTime(t)) => Some(rs::get_time::<Oracle>(g.target(t))),
            (Side::C, Op::ForceStop(t)) => {
                c::oracle_obs_transition_force_stop(g.target(t));
                None
            }
            (Side::Rust, Op::ForceStop(t)) => {
                rs::force_stop::<Oracle>(g.target(t));
                None
            }
            (Side::C, Op::CalcTime(ts)) => locked(&|| Some(c::oracle_transition_calc_time(tr, ts))),
            (Side::Rust, Op::CalcTime(ts)) => locked(&|| Some(rs::calc_time::<Oracle>(tr, ts))),
            (Side::C, Op::GetVideoTime) => {
                locked(&|| Some(c::oracle_transition_get_video_time(tr)))
            }
            (Side::Rust, Op::GetVideoTime) => locked(&|| Some(rs::get_video_time::<Oracle>(tr))),
            (Side::C, Op::Stop) => locked(&|| {
                c::oracle_transition_stop(tr);
                None
            }),
            (Side::Rust, Op::Stop) => locked(&|| {
                rs::stop::<Oracle>(tr);
                None
            }),
            (Side::C, Op::HandleStop) => {
                c::oracle_transition_handle_stop(tr);
                None
            }
            (Side::Rust, Op::HandleStop) => {
                rs::handle_stop::<Oracle>(tr);
                None
            }
        }
    }
}

/// Runs `ops` on a fresh graph through `side`, a snapshot after each.
fn trace_side(init: &Init, ops: &[Op], side: Side) -> Vec<Snapshot> {
    let g = Graph::new(init);
    let mut out = Vec::new();
    for &op in ops {
        if side == Side::C && op.checks_argument() == Some(Target::Null) {
            // The C crashes on NULL: nothing to compare, the Rust side is
            // checked against the graph as it stood.
            out.push(snapshot(&g, None));
            continue;
        }
        let r = run(&g, side, op);
        out.push(snapshot(&g, r));
    }
    out
}

/// The C's snapshot without the trace entries at `skip`: what the C did on
/// a source that is not a transition, which the Rust side never does.
fn without(s: &Snapshot, skip: &[usize]) -> Snapshot {
    let mut s = s.clone();
    s.trace = s
        .trace
        .iter()
        .enumerate()
        .filter(|(i, _)| !skip.contains(i))
        .map(|(_, e)| *e)
        .collect();
    s
}

/// Checks `ops` on both sides: equal after every call, but for the
/// argument checks, which leave the Rust side as it stood and return 0,
/// and which the C side is checked against on its own.
fn check(init: &Init, ops: &[Op]) {
    let _graph = graph_lock();
    let c_side = trace_side(init, ops, Side::C);
    let r_side = trace_side(init, ops, Side::Rust);
    let mut before = {
        let g = Graph::new(init);
        snapshot(&g, None)
    };
    let mut c_traced = before.trace.len();
    let mut skip = Vec::new();
    for (i, &op) in ops.iter().enumerate() {
        let (cs, rs_) = (&c_side[i], &r_side[i]);
        let ctx = || format!("call {i} {op:?} of {ops:?} on {init:?}");
        match op.checks_argument() {
            None => assert_eq!(&without(cs, &skip), rs_, "{}", ctx()),
            Some(target) => {
                let mut want = before.clone();
                want.result = matches!(op, Op::GetTime(_)).then_some(0.0f32.to_bits());
                assert_eq!(rs_, &want, "{}", ctx());
                if target == Target::NotTr {
                    // The C reads the input's zeroed clock (auto, no
                    // duration: 1 once the frame time is past 0), or calls
                    // its callback and signals the stop on it, and changes
                    // nothing else.
                    let new: Vec<_> = cs.trace[c_traced..].to_vec();
                    let mut rest = without(cs, &skip);
                    rest.trace.truncate(rest.trace.len() - new.len());
                    rest.result = want.result;
                    if let Op::GetTime(_) = op {
                        let t = if init.video_time == 0 { 0.0f32 } else { 1.0 };
                        assert_eq!(cs.result, Some(t.to_bits()), "{}", ctx());
                        assert_eq!(new, vec![(c::VIDEO_TIME, Some(TR), None, 0)], "{}", ctx());
                    } else {
                        let mut calls = Vec::new();
                        if init.plugin {
                            calls.push((c::PLUGIN, Some(NOT_TR), None, 0));
                        }
                        calls.push((sw::SIGNAL, Some(NOT_TR), None, 2));
                        assert_eq!(new, calls, "{}", ctx());
                    }
                    assert_eq!(rest, want, "{}", ctx());
                    skip.extend(c_traced..cs.trace.len());
                }
            }
        }
        before = rs_.clone();
        c_traced = cs.trace.len();
    }
}

fn slot(source: Option<usize>, active: bool) -> Slot {
    Slot { source, active }
}

fn base(slots: [Slot; 2], plugin: bool) -> Init {
    Init {
        slots,
        mode: AUTO,
        manual_val: 0.25,
        start_time: 1_000,
        duration: 4_000,
        video_time: 2_000,
        plugin,
    }
}

/// Every slot state: empty, or a child, active or not.
fn slot_states(child: usize) -> [Slot; 3] {
    [
        slot(None, false),
        slot(Some(child), false),
        slot(Some(child), true),
    ]
}

const ALL_OPS: [Op; 10] = [
    Op::GetTime(Target::Tr),
    Op::GetTime(Target::NotTr),
    Op::GetTime(Target::Null),
    Op::ForceStop(Target::Tr),
    Op::ForceStop(Target::NotTr),
    Op::ForceStop(Target::Null),
    Op::CalcTime(3_000),
    Op::GetVideoTime,
    Op::Stop,
    Op::HandleStop,
];

#[test]
fn every_call_on_every_slot_state() {
    for a in slot_states(CH_A) {
        for b in slot_states(CH_B) {
            for plugin in [false, true] {
                let init = base([a, b], plugin);
                for op in ALL_OPS {
                    check(&init, &[op]);
                }
                // A stop, then another (slot B is empty by then), then the
                // reads and the force stop.
                check(
                    &init,
                    &[
                        Op::Stop,
                        Op::HandleStop,
                        Op::Stop,
                        Op::GetTime(Target::Tr),
                        Op::ForceStop(Target::Tr),
                    ],
                );
            }
        }
    }
}

#[test]
fn the_same_child_in_both_slots() {
    for (a, b) in [(true, true), (true, false), (false, true), (false, false)] {
        let init = base([slot(Some(CH_A), a), slot(Some(CH_A), b)], true);
        check(&init, &[Op::Stop, Op::Stop, Op::ForceStop(Target::Tr)]);
    }
}

#[test]
fn the_time_in_each_mode_and_phase() {
    let clocks = [
        // (start, duration, frame time): before, at, in, at the end of and
        // past a start, no duration, and the ends of the range.
        (1_000, 4_000, 0),
        (1_000, 4_000, 1_000),
        (1_000, 4_000, 1_001),
        (1_000, 4_000, 3_000),
        (1_000, 4_000, 5_000),
        (1_000, 4_000, u64::MAX),
        (0, 0, 0),
        (0, 0, 1),
        (u64::MAX, u64::MAX, u64::MAX),
        (1, u64::MAX, u64::MAX),
        (1_000, 1_000_000_000, 1_001),
    ];
    for mode in [AUTO, MANUAL, -1, 2, 3, c_int::MIN, c_int::MAX] {
        for manual_val in [0.0, 0.5, 1.0, -0.0, f32::NAN, f32::INFINITY] {
            for (start_time, duration, video_time) in clocks {
                let init = Init {
                    mode,
                    manual_val,
                    start_time,
                    duration,
                    video_time,
                    ..base([slot(Some(CH_A), true), slot(Some(CH_B), true)], true)
                };
                check(
                    &init,
                    &[
                        Op::GetTime(Target::Tr),
                        Op::GetVideoTime,
                        Op::CalcTime(0),
                        Op::CalcTime(start_time.wrapping_add(1)),
                        Op::CalcTime(start_time.wrapping_add(duration / 2)),
                        Op::CalcTime(u64::MAX),
                    ],
                );
            }
        }
    }
}

#[test]
fn the_argument_checks_differ_from_c_on_purpose() {
    for plugin in [false, true] {
        for video_time in [0, 2_000] {
            let init = Init {
                video_time,
                ..base([slot(Some(CH_A), true), slot(Some(CH_B), false)], plugin)
            };
            for op in [
                Op::GetTime(Target::Null),
                Op::GetTime(Target::NotTr),
                Op::ForceStop(Target::Null),
                Op::ForceStop(Target::NotTr),
            ] {
                check(&init, &[op, Op::GetTime(Target::Tr), Op::Stop]);
            }
        }
    }
}

fn slot_strategy() -> impl Strategy<Value = Slot> {
    (
        prop::option::of(prop_oneof![Just(CH_A), Just(CH_B)]),
        any::<bool>(),
    )
        .prop_map(|(source, active)| Slot { source, active })
}

fn init_strategy() -> impl Strategy<Value = Init> {
    (
        [slot_strategy(), slot_strategy()],
        prop_oneof![
            Just(AUTO),
            Just(MANUAL),
            Just(-1),
            Just(2),
            Just(c_int::MIN),
            Just(c_int::MAX),
            any::<c_int>()
        ],
        any::<f32>(),
        prop_oneof![0u64..10_000, any::<u64>()],
        prop_oneof![Just(0u64), 1u64..10_000, any::<u64>()],
        prop_oneof![0u64..20_000, any::<u64>()],
        any::<bool>(),
    )
        .prop_map(
            |(slots, mode, manual_val, start_time, duration, video_time, plugin)| Init {
                slots,
                mode,
                manual_val,
                start_time,
                duration,
                video_time,
                plugin,
            },
        )
}

fn op_strategy() -> impl Strategy<Value = Op> {
    let target = prop_oneof![
        6 => Just(Target::Tr),
        1 => Just(Target::NotTr),
        1 => Just(Target::Null),
    ];
    prop_oneof![
        target.clone().prop_map(Op::GetTime),
        target.prop_map(Op::ForceStop),
        prop_oneof![0u64..20_000, any::<u64>()].prop_map(Op::CalcTime),
        Just(Op::GetVideoTime),
        Just(Op::Stop),
        Just(Op::HandleStop),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases(256)))]

    #[test]
    fn random_graphs_and_calls(
        init in init_strategy(),
        ops in prop::collection::vec(op_strategy(), 1..8),
    ) {
        check(&init, &ops);
    }
}
