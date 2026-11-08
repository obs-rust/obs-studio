//! Tier 3 for `obs_source_seam::ffi::source_transition_switch` against the
//! original `libobs/obs-source-transition-switch.c.h`
//! (`obs_c_oracle::source_transition_switch`, renamed `oracle_*`).
//!
//! Both sides run on the same graph, built afresh for each: a test
//! transition with a recording plugin `transition_start`, a source that is
//! not a transition, three children and two scenes whose full trees hold the
//! transition, one as a child and one as a parent (zeroed `struct
//! obs_source` with a type, a name and both transition mutexes), each with a
//! reference count, and an active-child link count per parent and child.
//! The C calls the oracle's recording
//! stand-ins for `obs_source_get_ref()`, `obs_source_release()`, the
//! active-child add and remove, `obs_source_enum_full_tree()`,
//! `obs_source_dosignal()`, `os_gettime_ns()` and the size and matrix
//! recalculation; the Rust shims are run with [`Oracle`], whose `Calls` are
//! those same stand-ins. Both reach the fields, the lock and the plugin
//! through the same C transition seam, as in the hybrid libobs. The plugin
//! re-enters once, when armed, through its own side's start or set.
//!
//! After every call the two must agree on what it returned, the
//! transition's slots, active flags, running flags, mode, manual value and
//! target and clock fields, which mutexes of each source are held, every
//! reference count and link count, and every call in order (the libobs
//! calls, the walks and what they visited, the signals, the clock reads,
//! the recalculations and the plugin's calls). The inputs are the C test's
//! sequences, every call on fixed graphs, and a proptest over random graphs
//! (children in either slot, active or not, running or idle, any mode
//! including ones outside the enum, clock, fixed duration and size, a child
//! going away, children whose add fails, a re-entering plugin) and
//! sequences of starts and sets (any mode, again including ones outside the
//! enum, and any duration, to no source, a child, slot A's or B's child, the
//! transition itself, a scene that holds it, or on a source that is not a
//! transition). In a child process, the warnings match the C's lines and
//! levels.
//!
//! No intentional differences. The order of the lock calls is not visible
//! here (the seam calls pthread itself); the Tier 1 tests pin it. Nor is the
//! `Done` admission: it needs a start on another thread between the finish
//! and the second check, so on one thread a start to the running
//! destination always finds it in slot A; the Tier 1 tests reach it with a
//! fake that changes the fields between two lock calls.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr;

use obs_c_oracle::parity::cases;
use obs_c_oracle::source_transition_settings::seam as settings_seam;
use obs_c_oracle::source_transition_slots::{self as slots, graph_lock, seam};
use obs_c_oracle::source_transition_switch::{self as c, EnumProc};
use obs_source_seam::ffi::source_transition_switch::{self as rs, Calls};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

/// `OBS_SOURCE_TYPE_INPUT`.
const INPUT: c_int = 0;
/// `OBS_SOURCE_TYPE_TRANSITION`.
const TRANSITION: c_int = 2;

/// The sources of a graph, in the order they are made.
const TR: usize = 0;
const NOT_TR: usize = 1;
const CH_A: usize = 2;
const CH_B: usize = 3;
const CH_C: usize = 4;
/// A scene whose full tree holds [`TR`].
const SCENE: usize = 5;
/// A scene whose full tree has [`TR`] as a parent.
const SCENE_2: usize = 6;
const SOURCES: usize = 7;

const NAMES: [&CStr; SOURCES] = [
    c"switch transition",
    c"an input",
    c"child a",
    c"child b",
    c"child c",
    c"a scene",
    c"another scene",
];

/// The oracle's stand-ins, as the Rust shims' libobs calls.
enum Oracle {}

// SAFETY: each is the stand-in the C oracle calls under the same name.
unsafe impl Calls for Oracle {
    unsafe fn get_ref(source: *mut c_void) -> *mut c_void {
        // SAFETY: see the trait.
        unsafe { slots::oracle_slots_get_ref(source) }
    }
    unsafe fn release(source: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { slots::oracle_slots_release(source) }
    }
    unsafe fn add_active_child(parent: *mut c_void, child: *mut c_void) -> bool {
        // SAFETY: see the trait.
        unsafe { slots::oracle_slots_add_active_child(parent, child) }
    }
    unsafe fn remove_active_child(parent: *mut c_void, child: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { slots::oracle_slots_remove_active_child(parent, child) }
    }
    unsafe fn get_name(source: *const c_void) -> *const c_char {
        // SAFETY: see the trait.
        unsafe { slots::oracle_slots_get_name(source) }
    }
    unsafe fn enum_full_tree(source: *mut c_void, callback: EnumProc, param: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { c::oracle_switch_enum_full_tree(source, callback, param) }
    }
    unsafe fn now() -> u64 {
        // SAFETY: see the trait.
        unsafe { c::oracle_switch_now() }
    }
    unsafe fn dosignal(
        transition: *mut c_void,
        signal_obs: *const c_char,
        signal_source: *const c_char,
    ) {
        // SAFETY: see the trait.
        unsafe { c::oracle_switch_dosignal(transition, signal_obs, signal_source) }
    }
    unsafe fn recalculate_size(transition: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { c::oracle_switch_recalculate_size(transition) }
    }
    unsafe fn recalculate_matrices(transition: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { c::oracle_switch_recalculate_matrices(transition) }
    }
}

/// The Rust side's re-entry: its own start.
unsafe extern "C" fn rust_start(
    transition: *mut c_void,
    mode: c_int,
    duration_ms: u32,
    dest: *mut c_void,
) -> bool {
    // SAFETY: the plugin hands back the live test transition and a source
    // of the graph (or NULL).
    unsafe { rs::start::<Oracle>(transition, mode, duration_ms, dest) }
}

/// The Rust side's re-entry: its own set.
unsafe extern "C" fn rust_set(transition: *mut c_void, source: *mut c_void) {
    // SAFETY: as for [`rust_start`].
    unsafe { rs::set::<Oracle>(transition, source) }
}

/// The C side's re-entry: the oracle's start, as `int` for its enum.
unsafe extern "C" fn c_start(
    transition: *mut c_void,
    mode: c_int,
    duration_ms: u32,
    dest: *mut c_void,
) -> bool {
    // SAFETY: as for [`rust_start`].
    unsafe { c::oracle_obs_transition_start(transition, mode, duration_ms, dest) }
}

/// The C side's re-entry: the oracle's set.
unsafe extern "C" fn c_set(transition: *mut c_void, source: *mut c_void) {
    // SAFETY: as for [`rust_start`].
    unsafe { c::oracle_obs_transition_set(transition, source) }
}

/// One slot of the transition: which source it holds and whether it is
/// active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Slot {
    source: Option<usize>,
    active: bool,
}

const EMPTY: Slot = Slot {
    source: None,
    active: false,
};

fn held(source: usize, active: bool) -> Slot {
    Slot {
        source: Some(source),
        active,
    }
}

/// What the plugin does the first time it is called.
#[derive(Debug, Clone, Copy)]
enum Reenter {
    Start(c_int, u32, Option<usize>),
    Set(Option<usize>),
}

/// How a graph starts.
#[derive(Debug, Clone)]
struct Init {
    slots: [Slot; 2],
    video: bool,
    audio: bool,
    mode: c_int,
    manual: (f32, f32),
    start_time: u64,
    duration: u64,
    fixed: Option<u32>,
    size: (u32, u32),
    /// A child whose last reference is gone.
    dying: Option<usize>,
    /// Children whose add to the transition fails.
    refuses: Vec<usize>,
    reenter: Option<Reenter>,
}

impl Init {
    /// As `make_transition()`: child A in an active slot A, idle.
    fn holding_a() -> Self {
        Self {
            slots: [held(CH_A, true), EMPTY],
            video: false,
            audio: false,
            mode: 0,
            manual: (0.25, 0.75),
            start_time: 5,
            duration: 7,
            fixed: None,
            size: (0, 0),
            dying: None,
            refuses: Vec::new(),
            reenter: None,
        }
    }
}

/// One call.
#[derive(Debug, Clone, Copy)]
enum Op {
    /// `obs_transition_start(target, mode, duration_ms, dest)`; the target
    /// is [`TR`], [`NOT_TR`] or NULL.
    Start(Option<usize>, c_int, u32, Option<usize>),
    /// `obs_transition_set(target, source)`.
    Set(Option<usize>, Option<usize>),
}

/// A graph's sources.
struct Graph {
    src: [*mut c_void; SOURCES],
}

impl Graph {
    /// Builds `init` on a fresh oracle graph, its re-entry through `side`.
    fn new(init: &Init, side: Side) -> Self {
        let mut src = [ptr::null_mut(); SOURCES];
        // SAFETY: the oracle's constructors and the seam setters on its
        // live sources; the caller holds the graph lock.
        unsafe {
            slots::oracle_slots_reset();
            c::oracle_switch_reset();
            for (i, s) in src.iter_mut().enumerate() {
                let kind = if i == TR { TRANSITION } else { INPUT };
                *s = slots::oracle_slots_source_new(kind, NAMES[i].as_ptr());
                assert!(!s.is_null());
            }
            let tr = src[TR];
            c::oracle_switch_plugin(tr);
            c::oracle_switch_tree_add(src[SCENE], src[SCENE], src[CH_C]);
            c::oracle_switch_tree_add(src[SCENE], src[CH_C], tr);
            c::oracle_switch_tree_add(src[CH_B], src[CH_B], src[CH_C]);
            c::oracle_switch_tree_add(src[SCENE_2], tr, src[CH_A]);
            slots::oracle_slots_refuse(tr, src[SCENE]);
            slots::oracle_slots_refuse(tr, src[SCENE_2]);
            for &r in &init.refuses {
                slots::oracle_slots_refuse(tr, src[r]);
            }
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
            seam::obs_source_transition_seam_set_transitioning_video(tr, init.video);
            seam::obs_source_transition_seam_set_transitioning_audio(tr, init.audio);
            seam::obs_source_transition_seam_set_mode(tr, init.mode);
            seam::obs_source_transition_seam_set_manual_val(tr, init.manual.0);
            seam::obs_source_transition_seam_set_manual_target(tr, init.manual.1);
            seam::obs_source_transition_seam_set_start_time(tr, init.start_time);
            seam::obs_source_transition_seam_set_duration(tr, init.duration);
            settings_seam::obs_source_transition_seam_set_use_fixed_duration(
                tr,
                init.fixed.is_some(),
            );
            settings_seam::obs_source_transition_seam_set_fixed_duration(
                tr,
                init.fixed.unwrap_or(1234),
            );
            settings_seam::obs_source_transition_seam_set_cx(tr, init.size.0);
            settings_seam::obs_source_transition_seam_set_cy(tr, init.size.1);
            if let Some(d) = init.dying {
                slots::oracle_slots_set_refs(src[d], 0);
            }
            if let Some(re) = init.reenter {
                let (start, set): (c::StartFn, c::SetFn) = match side {
                    Side::C => (c_start, c_set),
                    Side::Rust => (rust_start, rust_set),
                };
                let (kind, mode, ms, dest) = match re {
                    Reenter::Start(m, ms, d) => (1, m, ms, d),
                    Reenter::Set(d) => (2, 0, 0, d),
                };
                let dest = dest.map_or(ptr::null_mut(), |d| src[d]);
                c::oracle_switch_reenter(kind, mode, ms, dest, start, set);
            }
        }
        Self { src }
    }

    fn id(&self, p: *mut c_void) -> Option<usize> {
        (!p.is_null()).then(|| self.src.iter().position(|s| *s == p).unwrap_or(99))
    }

    fn ptr(&self, id: Option<usize>) -> *mut c_void {
        id.map_or(ptr::null_mut(), |i| self.src[i])
    }
}

/// What a graph looks like after a call.
#[derive(Debug, Clone, PartialEq)]
struct Snapshot {
    returned: Option<bool>,
    slots: [Slot; 2],
    video: bool,
    audio: bool,
    mode: c_int,
    /// The manual value and target, as bits.
    manual: (u32, u32),
    start_time: u64,
    duration: u64,
    /// Whether each source's `transition_mutex` is free.
    unlocked: Vec<bool>,
    refs: Vec<i64>,
    links: Vec<i64>,
    calls: Vec<(c_int, Option<usize>, Option<usize>, c_int)>,
}

fn snapshot(g: &Graph, returned: Option<bool>) -> Snapshot {
    let tr = g.src[TR];
    // SAFETY: the seam getters, the trylocks and the oracle's readers on the
    // graph's live sources, under the graph lock.
    unsafe {
        let mut s = Snapshot {
            returned,
            slots: [0, 1].map(|idx| Slot {
                source: g.id(seam::obs_source_transition_seam_get_sources(tr, idx)),
                active: seam::obs_source_transition_seam_get_source_active(tr, idx),
            }),
            video: seam::obs_source_transition_seam_get_transitioning_video(tr),
            audio: seam::obs_source_transition_seam_get_transitioning_audio(tr),
            mode: seam::obs_source_transition_seam_get_mode(tr),
            manual: (
                seam::obs_source_transition_seam_get_manual_val(tr).to_bits(),
                seam::obs_source_transition_seam_get_manual_target(tr).to_bits(),
            ),
            start_time: seam::obs_source_transition_seam_get_start_time(tr),
            duration: seam::obs_source_transition_seam_get_duration(tr),
            unlocked: Vec::new(),
            refs: Vec::new(),
            links: Vec::new(),
            calls: Vec::new(),
        };
        for &p in &g.src {
            let got = seam::obs_source_transition_seam_trylock(p) == 0;
            if got {
                seam::obs_source_transition_seam_unlock(p);
            }
            s.unlocked.push(got);
            s.refs.push(slots::oracle_slots_refs(p));
            s.links.push(slots::oracle_slots_link(tr, p));
        }
        for i in 0..slots::oracle_slots_traced() {
            let (mut kind, mut a, mut b, mut result) = (0, ptr::null_mut(), ptr::null_mut(), 0);
            slots::oracle_slots_event(i, &raw mut kind, &raw mut a, &raw mut b, &raw mut result);
            s.calls.push((kind, g.id(a), g.id(b), result));
        }
        s
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    C,
    Rust,
}

/// Runs `ops` on a graph built from `init` on one side; a snapshot after
/// every call.
fn run_side(side: Side, init: &Init, ops: &[Op]) -> Vec<Snapshot> {
    let g = Graph::new(init, side);
    let mut out = Vec::new();
    for &op in ops {
        // SAFETY: the graph's live sources, or NULL, which both check.
        let returned =
            unsafe {
                match (op, side) {
                    (Op::Start(t, mode, ms, dest), Side::C) => Some(
                        c::oracle_obs_transition_start(g.ptr(t), mode, ms, g.ptr(dest)),
                    ),
                    (Op::Start(t, mode, ms, dest), Side::Rust) => {
                        Some(rs::start::<Oracle>(g.ptr(t), mode, ms, g.ptr(dest)))
                    }
                    (Op::Set(t, source), Side::C) => {
                        c::oracle_obs_transition_set(g.ptr(t), g.ptr(source));
                        None
                    }
                    (Op::Set(t, source), Side::Rust) => {
                        rs::set::<Oracle>(g.ptr(t), g.ptr(source));
                        None
                    }
                }
            };
        out.push(snapshot(&g, returned));
    }
    // SAFETY: frees the graph's sources, under the graph lock.
    unsafe {
        slots::oracle_slots_reset();
        c::oracle_switch_reset();
    }
    out
}

fn run(init: &Init, ops: &[Op]) -> Result<(), TestCaseError> {
    let _g = graph_lock();
    let c_side = run_side(Side::C, init, ops);
    let rust_side = run_side(Side::Rust, init, ops);
    prop_assert_eq!(c_side.len(), rust_side.len());
    for (i, (c_snap, r_snap)) in c_side.iter().zip(&rust_side).enumerate() {
        prop_assert_eq!(
            c_snap,
            r_snap,
            "after step {} of {:?} on {:?}",
            i,
            ops,
            init
        );
    }
    Ok(())
}

fn check(init: &Init, ops: &[Op]) {
    run(init, ops).unwrap_or_else(|e| panic!("{e}"));
}

const AUTO: c_int = 0;
const MANUAL: c_int = 1;

fn start(dest: Option<usize>) -> Op {
    Op::Start(Some(TR), AUTO, 60_000, dest)
}

fn set(source: Option<usize>) -> Op {
    Op::Set(Some(TR), source)
}

/// The cases of `test_source_transition_switch.c`, each from
/// `make_transition()`.
#[test]
fn the_c_tests_sequences() {
    let (a, b, ch) = (Some(CH_A), Some(CH_B), Some(CH_C));
    let init = Init::holding_a();
    // start_from_idle
    check(&init, &[start(b)]);
    // start_to_source_a_idle_is_refused
    check(
        &init,
        &[
            start(a),
            Op::Start(Some(TR), MANUAL, 0, a),
            set(None),
            start(None),
        ],
    );
    // start_to_source_a_while_running
    check(&init, &[start(b), start(a), start(ch)]);
    // start_to_running_destination
    check(
        &init,
        &[
            Op::Start(Some(TR), MANUAL, 0, b),
            Op::Start(Some(TR), MANUAL, 0, b),
            start(ch),
            start(ch),
            Op::Start(Some(TR), MANUAL, 0, a),
            start(a),
        ],
    );
    // start_to_no_destination
    check(&init, &[start(None), start(None)]);
    // fixed_duration_replaces_the_asked_one
    let mut fixed = init.clone();
    fixed.fixed = Some(200);
    check(&fixed, &[Op::Start(Some(TR), AUTO, 600_000, b), set(a)]);
    // reentering_callback
    let mut reenter = init.clone();
    reenter.reenter = Some(Reenter::Start(AUTO, 60_000, ch));
    check(&reenter, &[start(b)]);
    // set_clears_both_slots
    check(
        &init,
        &[start(b), set(ch), set(a), set(None), set(a), set(Some(TR))],
    );
    // active_children_follow_the_slots
    check(&init, &[start(b), start(ch), set(a), set(None)]);
}

fn every_op() -> Vec<Op> {
    let dests = [
        None,
        Some(TR),
        Some(NOT_TR),
        Some(CH_A),
        Some(CH_B),
        Some(CH_C),
        Some(SCENE),
        Some(SCENE_2),
    ];
    let mut ops = vec![
        Op::Start(None, AUTO, 1, Some(CH_A)),
        Op::Start(Some(NOT_TR), AUTO, 1, Some(CH_A)),
        Op::Set(None, Some(CH_A)),
        Op::Set(Some(NOT_TR), Some(CH_A)),
    ];
    for d in dests {
        ops.push(Op::Start(Some(TR), AUTO, 300, d));
        ops.push(Op::Start(Some(TR), MANUAL, 0, d));
        // Modes outside the enum: stored and compared as the int they are.
        for mode in OUT_OF_RANGE_MODES {
            ops.push(Op::Start(Some(TR), mode, 300, d));
        }
        ops.push(Op::Set(Some(TR), d));
    }
    ops
}

fn fixed_inits() -> Vec<Init> {
    let base = Init::holding_a();
    let mut inits = vec![
        base.clone(),
        // Running from A to B, both active.
        Init {
            slots: [held(CH_A, true), held(CH_B, true)],
            video: true,
            audio: true,
            ..base.clone()
        },
        // Running audio only, manual, slot B inactive.
        Init {
            slots: [held(CH_A, true), held(CH_B, false)],
            audio: true,
            mode: MANUAL,
            ..base.clone()
        },
        // Running video only to nothing.
        Init {
            slots: [held(CH_A, true), EMPTY],
            video: true,
            ..base.clone()
        },
        // Empty and idle, with a size and a fixed duration.
        Init {
            slots: [EMPTY; 2],
            size: (1920, 1080),
            fixed: Some(250),
            ..base.clone()
        },
        // Only the height set: the size is still recalculated.
        Init {
            size: (1920, 0),
            ..base.clone()
        },
        // The same child in both slots, inactive.
        Init {
            slots: [held(CH_A, false), held(CH_A, false)],
            video: true,
            ..base.clone()
        },
    ];
    for d in [CH_A, CH_B] {
        inits.push(Init {
            dying: Some(d),
            ..base.clone()
        });
    }
    inits.push(Init {
        refuses: vec![CH_B, CH_A],
        ..base.clone()
    });
    // A stored mode outside the enum, running and idle: the second check
    // compares it with the asked one as an int.
    for mode in OUT_OF_RANGE_MODES {
        inits.push(Init {
            slots: [held(CH_A, true), held(CH_B, true)],
            video: true,
            mode,
            ..base.clone()
        });
        inits.push(Init {
            mode,
            ..base.clone()
        });
    }
    // Idle with an active slot B: a start hands the slot over, adding the
    // destination before it removes the old child; a refused add removes
    // nothing.
    inits.push(Init {
        slots: [held(CH_A, true), held(CH_B, true)],
        refuses: vec![CH_C],
        ..base.clone()
    });
    for reenter in [
        Reenter::Start(AUTO, 9, Some(CH_C)),
        Reenter::Start(MANUAL, 9, Some(CH_A)),
        Reenter::Start(AUTO, 9, None),
        Reenter::Start(AUTO, 9, Some(SCENE)),
        Reenter::Set(Some(CH_C)),
        Reenter::Set(None),
        Reenter::Set(Some(TR)),
    ] {
        inits.push(Init {
            reenter: Some(reenter),
            ..base.clone()
        });
    }
    inits
}

#[test]
fn every_call_on_fixed_graphs() {
    for init in fixed_inits() {
        for op in every_op() {
            check(&init, &[op]);
        }
        check(&init, &every_op());
    }
}

/// Modes no `enum obs_transition_mode` value has: a plugin or script can
/// hand any `int`, and the C stores and compares it unchecked.
const OUT_OF_RANGE_MODES: [c_int; 5] = [-1, 2, 3, c_int::MIN, c_int::MAX];

/// Auto and manual most of the time, otherwise any `int`.
fn gen_mode() -> impl Strategy<Value = c_int> {
    prop_oneof![
        6 => 0..2,
        1 => proptest::sample::select(OUT_OF_RANGE_MODES.to_vec()),
        1 => any::<c_int>(),
    ]
}

fn gen_child() -> impl Strategy<Value = Option<usize>> {
    prop_oneof![
        Just(None),
        Just(Some(TR)),
        Just(Some(NOT_TR)),
        (CH_A..=CH_C).prop_map(Some),
        Just(Some(SCENE)),
        Just(Some(SCENE_2)),
    ]
}

fn gen_slot() -> impl Strategy<Value = Slot> {
    (proptest::option::of(CH_A..=SCENE_2), any::<bool>())
        .prop_map(|(source, active)| Slot { source, active })
}

fn gen_reenter() -> impl Strategy<Value = Reenter> {
    prop_oneof![
        (gen_mode(), any::<u32>(), gen_child()).prop_map(|(m, ms, d)| Reenter::Start(m, ms, d)),
        gen_child().prop_map(Reenter::Set),
    ]
}

fn gen_target() -> impl Strategy<Value = Option<usize>> {
    prop_oneof![8 => Just(Some(TR)), 1 => Just(Some(NOT_TR)), 1 => Just(None)]
}

fn gen_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => (gen_target(), gen_mode(), any::<u32>(), gen_child())
            .prop_map(|(t, m, ms, d)| Op::Start(t, m, ms, d)),
        1 => (gen_target(), gen_child()).prop_map(|(t, s)| Op::Set(t, s)),
    ]
}

prop_compose! {
    fn gen_init()(
        slots in proptest::array::uniform2(gen_slot()),
        video in any::<bool>(),
        audio in any::<bool>(),
        mode in gen_mode(),
        manual in (0.0f32..=1.0, 0.0f32..=1.0),
        start_time in any::<u64>(),
        duration in any::<u64>(),
        fixed in proptest::option::of(any::<u32>()),
        size in (prop_oneof![Just(0u32), any::<u32>()], prop_oneof![Just(0u32), any::<u32>()]),
        dying in proptest::option::of(CH_A..=SCENE_2),
        refuses in proptest::collection::vec(CH_A..=CH_C, 0..3),
        reenter in proptest::option::of(gen_reenter()),
    ) -> Init {
        Init {
            slots, video, audio, mode, manual, start_time, duration, fixed, size, dying,
            refuses, reenter,
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases(256)))]

    /// Any graph and any starts and sets: the same results, state,
    /// references, links and calls after each.
    #[test]
    fn any_graph_and_calls(init in gen_init(), ops in proptest::collection::vec(gen_op(), 1..8)) {
        run(&init, &ops)?;
    }
}

// ---------------------------------------------------------------------------
// The warnings

#[repr(C)]
struct Rec {
    calls: c_int,
    level: c_int,
    message: [c_char; 256],
}

unsafe extern "C" {
    fn base_test_record_ptr() -> *mut c_void;
    fn base_test_record_only_this_thread();
}

/// The process-wide log handler, pointed at a record while it lives.
struct Capture {
    rec: Box<Rec>,
    saved: (*mut c_void, *mut c_void),
}

impl Capture {
    fn arm() -> Self {
        let mut saved = (ptr::null_mut(), ptr::null_mut());
        let mut rec = Box::new(Rec {
            calls: 0,
            level: -1,
            message: [0; 256],
        });
        // SAFETY: the record outlives the handler (restored in drop).
        unsafe {
            base_get_log_handler(&raw mut saved.0, &raw mut saved.1);
            base_test_record_only_this_thread();
            base_set_log_handler(base_test_record_ptr(), (&raw mut *rec).cast());
        }
        Self { rec, saved }
    }

    /// The calls since the last take, the level and text of the last line.
    fn take(&mut self) -> (c_int, c_int, String) {
        // SAFETY: the recorder NUL-terminates what it writes; the record
        // starts zeroed.
        let text = unsafe { CStr::from_ptr(self.rec.message.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        let line = (self.rec.calls, self.rec.level, text);
        self.rec.calls = 0;
        self.rec.level = -1;
        self.rec.message[0] = 0;
        line
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        // SAFETY: the handler read in `arm`.
        unsafe { base_set_log_handler(self.saved.0, self.saved.1) };
    }
}

/// `LOG_WARNING`.
const LOG_WARNING: c_int = 200;

/// Each refusal logs one warning naming the transition and the source,
/// the same line from both sides; a start and set that go through log
/// nothing. Swaps the process-wide log handler, so it runs in a child
/// process.
#[test]
fn the_warnings_log_the_cs_lines() {
    if !crate::output::isolated(concat!(module_path!(), "::the_warnings_log_the_cs_lines")) {
        return;
    }
    let mut log = Capture::arm();
    let mut refusing = Init::holding_a();
    refusing.refuses = vec![CH_B];
    let cases: [(Init, Op, String); 6] = [
        (
            Init::holding_a(),
            start(Some(TR)),
            "obs_transition_start: transition 'switch transition' has recursive destination 'switch transition'".into(),
        ),
        (
            Init::holding_a(),
            start(Some(SCENE)),
            "obs_transition_start: transition 'switch transition' has recursive destination 'a scene'".into(),
        ),
        (
            refusing,
            start(Some(CH_B)),
            "obs_transition_start: transition 'switch transition' has recursive destination 'child b'".into(),
        ),
        (
            Init::holding_a(),
            set(Some(TR)),
            "obs_transition_set: cannot set transition 'switch transition' to source 'switch transition' due to infinite recursion".into(),
        ),
        (
            Init::holding_a(),
            set(Some(SCENE)),
            "obs_transition_set: cannot set transition 'switch transition' to source 'a scene' due to infinite recursion".into(),
        ),
        (
            Init::holding_a(),
            start(Some(SCENE_2)),
            "obs_transition_start: transition 'switch transition' has recursive destination 'another scene'".into(),
        ),
    ];
    for (init, op, want) in cases {
        let mut lines = Vec::new();
        for side in [Side::C, Side::Rust] {
            let _g = graph_lock();
            run_side(side, &init, &[op]);
            lines.push(log.take());
        }
        assert_eq!(lines[0], (1, LOG_WARNING, want), "the C's line");
        assert_eq!(lines[1], lines[0], "the Rust line");
    }
    for side in [Side::C, Side::Rust] {
        let _g = graph_lock();
        run_side(
            side,
            &Init::holding_a(),
            &[start(Some(CH_B)), set(Some(CH_C))],
        );
        assert_eq!(log.take().0, 0);
    }
}
