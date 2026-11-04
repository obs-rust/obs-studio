//! Tier 3 for `obs_source_seam::ffi::source_transition_access` against the
//! original `libobs/obs-source-transition-access.c.h`
//! (`obs_c_oracle::source_transition_access`, renamed `oracle_*`).
//!
//! Both sides run on the same graph, built afresh for each call: a test
//! transition, a source that is not a transition and three children (the
//! slots oracle's zeroed `struct obs_source` with a type, a name and both
//! transition mutexes), each with a reference count. The C calls the
//! oracle's recording stand-ins for `obs_source_get_ref()` and
//! `obs_transition_set()`; the Rust shims are run with [`Oracle`], whose
//! `Calls` are those same stand-ins. The C reaches the fields and the lock
//! directly, the Rust through the C transition seam, as in the hybrid
//! libobs. The graph is the slots oracle's, so every use holds its
//! `graph_lock`, the lock the slots parity suite holds too: a reset frees
//! every source.
//!
//! After each call the two must agree on the source returned, every slot,
//! the running flags, the mode and the manual target, torque and clamp of
//! both sources, whether each transition mutex is held, every reference
//! count, the `obs_source_get_ref()` calls in order and the
//! `obs_transition_set()` calls (with whether the mutex was held). The
//! inputs are every call on fixed graphs and a proptest over random slots,
//! flags, modes, children going away, torques and times. In a child
//! process, the getters' NULL line matches the C's.
//!
//! Intentional differences (bug fixes), and the only inputs the comparison
//! leaves out:
//! - `obs_transition_set_manual_torque()` and
//!   `obs_transition_set_manual_time()` on NULL: the C crashes on the lock;
//!   the Rust logs the setter's name at debug level and does nothing. The C
//!   is not run; the Rust must leave the graph as it was.
//! - The same two on a source that is not a transition: the C writes its
//!   manual fields (and may hand slot A to `obs_transition_set()`); the
//!   Rust does nothing. The Rust must match the C run with no call.
//!
//! The slot index is compared for the two slots only: past them, the C and
//! the seam both index past `transition_sources`.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr;

use obs_c_oracle::parity::cases;
use obs_c_oracle::source_transition_access as c;
use obs_c_oracle::source_transition_slots::{self as slots, graph_lock, seam};
use obs_source_seam::ffi::source_transition_access::{self as rs, Calls};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

/// `OBS_SOURCE_TYPE_INPUT`.
const INPUT: c_int = 0;
/// `OBS_SOURCE_TYPE_TRANSITION`.
const TRANSITION: c_int = 2;

/// The sources of a graph, in the order they are made.
const TR: usize = 0;
const NOT_TR: usize = 1;
const CHILD_A: usize = 2;
const CHILD_B: usize = 3;
const CHILD_C: usize = 4;
const SOURCES: usize = 5;

const NAMES: [&CStr; SOURCES] = [
    c"a transition",
    c"an input",
    c"child a",
    c"child b",
    c"child c",
];

/// The oracle's stand-ins, as the Rust shims' libobs calls.
enum Oracle {}

// SAFETY: each is the stand-in the C oracle calls under the same name.
unsafe impl Calls for Oracle {
    unsafe fn get_ref(source: *mut c_void) -> *mut c_void {
        // SAFETY: see the trait.
        unsafe { slots::oracle_slots_get_ref(source) }
    }
    unsafe fn transition_set(transition: *mut c_void, source: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { c::oracle_access_set(transition, source) }
    }
}

/// How the transition and the input start.
#[derive(Debug, Clone, Copy)]
struct Init {
    /// The slots of [`TR`] and [`NOT_TR`].
    slots: [[Option<usize>; 2]; 2],
    video: [bool; 2],
    audio: [bool; 2],
    mode: [c_int; 2],
    /// `(target, torque, clamp)`.
    manual: [(f32, f32, f32); 2],
    /// A child whose last reference is gone (`obs_source_get_ref()` gives
    /// NULL).
    dying: Option<usize>,
}

impl Init {
    fn new(slots: [Option<usize>; 2]) -> Self {
        Self {
            slots: [slots, [Some(CHILD_C), Some(CHILD_A)]],
            video: [false; 2],
            audio: [false; 2],
            mode: [0; 2],
            manual: [(0.125, 0.5, 0.25); 2],
            dying: None,
        }
    }
}

/// Which source a call gets: one of the graph's, or NULL.
type Target = Option<usize>;

/// One call.
#[derive(Debug, Clone, Copy)]
enum Op {
    Get(Target, c_int),
    Active(Target),
    Torque(Target, f32, f32),
    Time(Target, f32),
}

impl Op {
    /// The setters on NULL or a source that is not a transition: the
    /// intentional differences.
    fn fixed(self) -> bool {
        match self {
            Self::Torque(t, ..) | Self::Time(t, _) => t != Some(TR),
            Self::Get(..) | Self::Active(_) => false,
        }
    }
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
            c::oracle_access_reset();
            for (i, s) in src.iter_mut().enumerate() {
                let kind = if i == TR { TRANSITION } else { INPUT };
                *s = slots::oracle_slots_source_new(kind, NAMES[i].as_ptr());
                assert!(!s.is_null());
            }
            for (t, slots_of) in init.slots.iter().enumerate() {
                let s = src[t];
                for (idx, slot) in slots_of.iter().enumerate() {
                    let Some(child) = *slot else { continue };
                    seam::obs_source_transition_seam_set_sources(s, idx, src[child]);
                    let refs = slots::oracle_slots_refs(src[child]);
                    slots::oracle_slots_set_refs(src[child], refs + 1);
                }
                seam::obs_source_transition_seam_set_transitioning_video(s, init.video[t]);
                seam::obs_source_transition_seam_set_transitioning_audio(s, init.audio[t]);
                seam::obs_source_transition_seam_set_mode(s, init.mode[t]);
                let (target, torque, clamp) = init.manual[t];
                seam::obs_source_transition_seam_set_manual_target(s, target);
                seam::obs_source_transition_seam_set_manual_torque(s, torque);
                c::seam::obs_source_transition_seam_set_manual_clamp(s, clamp);
            }
            if let Some(d) = init.dying {
                slots::oracle_slots_set_refs(src[d], 0);
            }
        }
        Self { src }
    }

    fn id(&self, p: *mut c_void) -> Option<usize> {
        if p.is_null() {
            None
        } else {
            Some(self.src.iter().position(|s| *s == p).unwrap_or(99))
        }
    }

    fn ptr(&self, t: Target) -> *mut c_void {
        t.map_or(ptr::null_mut(), |t| self.src[t])
    }
}

/// One source's fields (the floats as bits).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fields {
    slots: [Option<usize>; 2],
    video: bool,
    audio: bool,
    mode: c_int,
    target: u32,
    torque: u32,
    clamp: u32,
    /// `transition_mutex` free.
    unlocked: bool,
}

/// What a graph looks like after a call.
#[derive(Debug, Clone, PartialEq)]
struct Snapshot {
    got: Option<usize>,
    fields: Vec<Fields>,
    refs: Vec<i64>,
    /// `obs_source_get_ref()`: the source and whether it gave it.
    get_refs: Vec<(Option<usize>, c_int)>,
    /// `obs_transition_set()`: the transition, the source and whether the
    /// mutex was held.
    sets: Vec<(Option<usize>, Option<usize>, bool)>,
}

fn snapshot(g: &Graph, got: *mut c_void) -> Snapshot {
    // SAFETY: the seam getters, the trylock and the oracles' readers on the
    // graph's live sources, under the graph lock.
    unsafe {
        let mut s = Snapshot {
            got: g.id(got),
            fields: Vec::new(),
            refs: Vec::new(),
            get_refs: Vec::new(),
            sets: Vec::new(),
        };
        for &p in &g.src {
            let unlocked = seam::obs_source_transition_seam_trylock(p) == 0;
            if unlocked {
                seam::obs_source_transition_seam_unlock(p);
            }
            s.fields.push(Fields {
                slots: [0, 1].map(|idx| g.id(seam::obs_source_transition_seam_get_sources(p, idx))),
                video: seam::obs_source_transition_seam_get_transitioning_video(p),
                audio: seam::obs_source_transition_seam_get_transitioning_audio(p),
                mode: seam::obs_source_transition_seam_get_mode(p),
                target: seam::obs_source_transition_seam_get_manual_target(p).to_bits(),
                torque: seam::obs_source_transition_seam_get_manual_torque(p).to_bits(),
                clamp: c::seam::obs_source_transition_seam_get_manual_clamp(p).to_bits(),
                unlocked,
            });
            s.refs.push(slots::oracle_slots_refs(p));
        }
        for i in 0..slots::oracle_slots_traced() {
            let (mut kind, mut a, mut b, mut result) = (0, ptr::null_mut(), ptr::null_mut(), 0);
            slots::oracle_slots_event(i, &raw mut kind, &raw mut a, &raw mut b, &raw mut result);
            assert_eq!(kind, slots::GET_REF, "only obs_source_get_ref() is traced");
            s.get_refs.push((g.id(a), result));
        }
        for i in 0..c::oracle_access_sets() {
            let (mut t, mut src, mut locked) = (ptr::null_mut(), ptr::null_mut(), false);
            c::oracle_access_set_call(i, &raw mut t, &raw mut src, &raw mut locked);
            s.sets.push((g.id(t), g.id(src), locked));
        }
        s
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    C,
    Rust,
    /// The C graph with no call made.
    Untouched,
}

/// Runs `op` on a graph built from `init` on one side.
fn run_side(side: Side, init: &Init, op: Op) -> Snapshot {
    let g = Graph::new(init);
    let mut got = ptr::null_mut();
    // SAFETY: the graph's live sources, or NULL where the side checks it;
    // the slot index is 0 or 1 for a transition.
    unsafe {
        match (op, side) {
            (_, Side::Untouched) => {}
            (Op::Get(t, target), Side::C) => {
                got = c::oracle_obs_transition_get_source(g.ptr(t), target);
            }
            (Op::Get(t, target), Side::Rust) => {
                got = rs::get_source::<Oracle>(g.ptr(t), target);
            }
            (Op::Active(t), Side::C) => got = c::oracle_obs_transition_get_active_source(g.ptr(t)),
            (Op::Active(t), Side::Rust) => got = rs::get_active_source::<Oracle>(g.ptr(t)),
            (Op::Torque(t, torque, clamp), Side::C) => {
                c::oracle_obs_transition_set_manual_torque(g.ptr(t), torque, clamp);
            }
            (Op::Torque(t, torque, clamp), Side::Rust) => {
                rs::set_manual_torque::<Oracle>(g.ptr(t), torque, clamp);
            }
            (Op::Time(t, time), Side::C) => {
                c::oracle_obs_transition_set_manual_time(g.ptr(t), time);
            }
            (Op::Time(t, time), Side::Rust) => rs::set_manual_time::<Oracle>(g.ptr(t), time),
        }
    }
    let snap = snapshot(&g, got);
    // SAFETY: frees the graph's sources, under the graph lock.
    unsafe {
        slots::oracle_slots_reset();
        c::oracle_access_reset();
    }
    snap
}

fn run(init: &Init, op: Op) -> Result<(), TestCaseError> {
    let _g = graph_lock();
    let want = run_side(if op.fixed() { Side::Untouched } else { Side::C }, init, op);
    let got = run_side(Side::Rust, init, op);
    prop_assert_eq!(want, got, "{:?} on {:?}", op, init);
    Ok(())
}

fn every_op() -> Vec<Op> {
    let mut ops = Vec::new();
    for t in [Some(TR), Some(NOT_TR), None] {
        for target in [0, 1] {
            ops.push(Op::Get(t, target));
        }
        ops.push(Op::Active(t));
        for (torque, clamp) in [(0.0, 0.0), (0.1, 1000.0), (-3.0, f32::NAN)] {
            ops.push(Op::Torque(t, torque, clamp));
        }
        for time in [0.0, -0.0, 0.25, 1.0, f32::NAN] {
            ops.push(Op::Time(t, time));
        }
    }
    ops
}

fn fixed_inits() -> Vec<Init> {
    let (a, b) = (Some(CHILD_A), Some(CHILD_B));
    let mut inits = Vec::new();
    for slots_of in [[a, b], [a, None], [None, b], [None, None], [a, a]] {
        for (video, audio) in [(false, false), (true, false), (false, true), (true, true)] {
            for mode in [0, 1] {
                let mut init = Init::new(slots_of);
                init.video = [video, video];
                init.audio = [audio, audio];
                init.mode = [mode, mode];
                inits.push(init);
            }
        }
    }
    for dying in [CHILD_A, CHILD_B] {
        let mut init = Init::new([a, b]);
        init.dying = Some(dying);
        init.mode = [1, 1];
        inits.push(init);
        init.video = [true, true];
        inits.push(init);
    }
    inits
}

#[test]
fn every_call_on_fixed_graphs() {
    for init in fixed_inits() {
        for op in every_op() {
            run(&init, op).unwrap_or_else(|e| panic!("{e}"));
        }
    }
}

/// The C test's manual sequence, call by call on fresh graphs: each time
/// sets the target, and only 0 in manual mode hands slot A on.
#[test]
fn the_c_tests_manual_times() {
    let mut init = Init::new([Some(CHILD_A), Some(CHILD_B)]);
    init.mode = [1, 1];
    init.video = [true, true];
    for op in [
        Op::Torque(Some(TR), 0.0, 0.0),
        Op::Time(Some(TR), 0.25),
        Op::Torque(Some(TR), 0.1, 1000.0),
        Op::Time(Some(TR), 0.75),
        Op::Time(Some(TR), 0.0),
        Op::Time(Some(TR), -0.0),
        Op::Time(Some(TR), f32::NAN),
    ] {
        run(&init, op).unwrap_or_else(|e| panic!("{e}"));
    }
}

/// The fixed inputs really differ from the C where the list above says,
/// and nowhere else: the C writes an input's manual fields.
#[test]
fn intentional_differences_are_real() {
    let _g = graph_lock();
    let mut init = Init::new([Some(CHILD_A), Some(CHILD_B)]);
    init.mode = [1, 1];
    let op = Op::Time(Some(NOT_TR), 0.0);
    let c_side = run_side(Side::C, &init, op);
    let untouched = run_side(Side::Untouched, &init, op);
    assert_ne!(c_side, untouched, "the C writes the input");
    assert_eq!(
        c_side.sets,
        [(Some(NOT_TR), Some(CHILD_C), false)],
        "and hands its slot A on"
    );
    let op = Op::Torque(Some(NOT_TR), 2.0, 3.0);
    assert_ne!(
        run_side(Side::C, &init, op),
        run_side(Side::Untouched, &init, op)
    );
}

fn gen_slot() -> impl Strategy<Value = Option<usize>> {
    // The children, the transition itself, and empty.
    prop_oneof![
        Just(None),
        Just(Some(TR)),
        (CHILD_A..SOURCES).prop_map(Some)
    ]
}

fn gen_float() -> impl Strategy<Value = f32> {
    prop_oneof![
        Just(0.0f32),
        Just(-0.0f32),
        Just(f32::NAN),
        Just(f32::INFINITY),
        -2.0f32..2.0,
    ]
}

fn gen_target() -> impl Strategy<Value = Target> {
    prop_oneof![
        3 => Just(Some(TR)),
        1 => Just(Some(NOT_TR)),
        1 => Just(None),
    ]
}

fn gen_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (gen_target(), 0..2).prop_map(|(t, i)| Op::Get(t, i)),
        gen_target().prop_map(Op::Active),
        (gen_target(), gen_float(), gen_float()).prop_map(|(t, a, b)| Op::Torque(t, a, b)),
        (gen_target(), gen_float()).prop_map(|(t, time)| Op::Time(t, time)),
    ]
}

prop_compose! {
    fn gen_init()(
        slots_of in proptest::array::uniform2(proptest::array::uniform2(gen_slot())),
        video in proptest::array::uniform2(any::<bool>()),
        audio in proptest::array::uniform2(any::<bool>()),
        mode in proptest::array::uniform2(0..3),
        manual in proptest::array::uniform2((gen_float(), gen_float(), gen_float())),
        dying in proptest::option::of(CHILD_A..SOURCES),
    ) -> Init {
        Init { slots: slots_of, video, audio, mode, manual, dying }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases(256)))]

    /// Any graph and any call: the same source, fields, references, locks
    /// and calls, but for the listed differences.
    #[test]
    fn any_graph_and_call(init in gen_init(), op in gen_op()) {
        run(&init, op)?;
    }
}

// ---------------------------------------------------------------------------
// The NULL lines

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

/// `LOG_DEBUG`.
const LOG_DEBUG: c_int = 400;

/// NULL: both getters log the C's line from both sides (the C's
/// `obs_transition_get_source` for both); the setters log their own names
/// (the intentional difference: the C crashes, so it is not run); a source
/// that is not a transition logs nothing. Swaps the process-wide log
/// handler, so it runs in a child process.
#[test]
fn null_lines() {
    if !crate::output::isolated(concat!(module_path!(), "::null_lines")) {
        return;
    }
    let mut log = Capture::arm();
    let init = Init::new([Some(CHILD_A), Some(CHILD_B)]);
    let getter = "obs_transition_get_source: Null 'transition' parameter".to_owned();
    for op in [Op::Get(None, 0), Op::Active(None)] {
        let _g = graph_lock();
        run_side(Side::C, &init, op);
        assert_eq!(log.take(), (1, LOG_DEBUG, getter.clone()), "the C's line");
        run_side(Side::Rust, &init, op);
        assert_eq!(log.take(), (1, LOG_DEBUG, getter.clone()), "the Rust line");
    }
    for (op, name) in [
        (
            Op::Torque(None, 1.0, 1.0),
            "obs_transition_set_manual_torque",
        ),
        (Op::Time(None, 0.0), "obs_transition_set_manual_time"),
    ] {
        let _g = graph_lock();
        run_side(Side::Rust, &init, op);
        let want = format!("{name}: Null 'transition' parameter");
        assert_eq!(log.take(), (1, LOG_DEBUG, want));
    }
    for op in [
        Op::Get(Some(NOT_TR), 0),
        Op::Active(Some(NOT_TR)),
        Op::Torque(Some(NOT_TR), 1.0, 1.0),
        Op::Time(Some(NOT_TR), 0.0),
    ] {
        let _g = graph_lock();
        run_side(Side::Rust, &init, op);
        assert_eq!(log.take().0, 0, "{op:?}");
    }
}
