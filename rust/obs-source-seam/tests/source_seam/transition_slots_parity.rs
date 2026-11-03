//! Tier 3 for `obs_source_seam::ffi::source_transition_slots` against the
//! original `libobs/obs-source-transition-slots.c.h`
//! (`obs_c_oracle::source_transition_slots`, renamed `oracle_*`).
//!
//! Both sides run on the same graph, built afresh for each: two test
//! transitions, a source that is not a transition and four children (zeroed
//! `struct obs_source` with a type, a name and both transition mutexes),
//! each with a reference count, and an active-child link count per parent
//! and child. The C calls the oracle's recording stand-ins for
//! `obs_source_get_ref()`, `obs_source_release()` and the active-child add
//! and remove; the Rust shims are run with [`Oracle`], whose `Calls` are
//! those same stand-ins. Both reach the fields and locks through the same C
//! transition seam, as in the hybrid libobs.
//!
//! After every call (and between a swap's begin and end) the two must agree
//! on every slot, active flag, running flag and clock field of every
//! source, every render target, which of the two mutexes of each source are
//! held, every reference count and link count, the calls in order, and the
//! children the enum's callback got (with the transition lock held). The
//! inputs are every call on fixed graphs and a proptest over random graphs
//! (children in any slot, active or not, a child going away, the new
//! transition or a scene that contains it in the old one's slots) and
//! sequences of calls. In a child process, the recursion warning matches
//! the C's line and level.
//!
//! No intentional differences. The order of the lock calls is not visible
//! here (the seam calls pthread itself); the Tier 1 tests pin it.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr;
use std::sync::{Mutex, MutexGuard, PoisonError};

use obs_c_oracle::parity::cases;
use obs_c_oracle::source_transition_slots::{self as c, seam};
use obs_source_seam::ffi::source_transition_slots::{self as rs, Calls};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

/// `OBS_SOURCE_TYPE_INPUT`.
const INPUT: c_int = 0;
/// `OBS_SOURCE_TYPE_TRANSITION`.
const TRANSITION: c_int = 2;

/// The sources of a graph, in the order they are made.
const OLD: usize = 0;
const NEW: usize = 1;
const NOT_TR: usize = 2;
const CHILD_A: usize = 3;
const CHILD_B: usize = 4;
const CHILD_C: usize = 5;
/// A scene that contains [`NEW`]: its add to `NEW` fails.
const SCENE: usize = 6;
const SOURCES: usize = 7;

const NAMES: [&CStr; SOURCES] = [
    c"old transition",
    c"new transition",
    c"an input",
    c"child a",
    c"child b",
    c"child c",
    c"a scene",
];

/// The oracle graph is process-wide.
static GRAPH: Mutex<()> = Mutex::new(());

fn graph_lock() -> MutexGuard<'static, ()> {
    GRAPH.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The oracle's stand-ins, as the Rust shims' libobs calls.
enum Oracle {}

// SAFETY: each is the stand-in the C oracle calls under the same name.
unsafe impl Calls for Oracle {
    unsafe fn get_ref(source: *mut c_void) -> *mut c_void {
        // SAFETY: see the trait.
        unsafe { c::oracle_slots_get_ref(source) }
    }
    unsafe fn release(source: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { c::oracle_slots_release(source) }
    }
    unsafe fn add_active_child(parent: *mut c_void, child: *mut c_void) -> bool {
        // SAFETY: see the trait.
        unsafe { c::oracle_slots_add_active_child(parent, child) }
    }
    unsafe fn remove_active_child(parent: *mut c_void, child: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { c::oracle_slots_remove_active_child(parent, child) }
    }
    unsafe fn get_name(source: *const c_void) -> *const c_char {
        // SAFETY: see the trait.
        unsafe { c::oracle_slots_get_name(source) }
    }
}

/// One slot of a transition: which source it holds and whether it is
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

/// How a graph starts.
#[derive(Debug, Clone)]
struct Init {
    /// The slots of [`OLD`], [`NEW`] and [`NOT_TR`].
    slots: [[Slot; 2]; 3],
    /// Running flags, clock and manual state of [`OLD`] and [`NEW`].
    video: [bool; 2],
    audio: [bool; 2],
    start_time: [u64; 2],
    duration: [u64; 2],
    mode: [c_int; 2],
    manual: [(f32, f32); 2],
    /// A child whose last reference is gone (`obs_source_get_ref()` gives
    /// NULL).
    dying: Option<usize>,
}

impl Init {
    fn running(old: [Slot; 2], new: [Slot; 2]) -> Self {
        Self {
            slots: [old, new, [EMPTY; 2]],
            video: [true, false],
            audio: [true, false],
            start_time: [1_000, 5],
            duration: [60_000_000_000, 7],
            mode: [0, 0],
            manual: [(0.25, 0.75), (0.0, 0.0)],
            dying: None,
        }
    }
}

fn held(source: usize, active: bool) -> Slot {
    Slot {
        source: Some(source),
        active,
    }
}

/// One call.
#[derive(Debug, Clone, Copy)]
enum Op {
    Clear(Option<usize>),
    Enum(usize),
    /// `obs_transition_swap_begin(dest, source)`, then `_end` on the pair.
    Swap(usize, usize),
}

/// A graph's sources.
struct Graph {
    src: [*mut c_void; SOURCES],
}

/// A render target's fake pointer: never dereferenced, only moved.
fn texrender(source: usize, idx: usize) -> *mut c_void {
    ptr::without_provenance_mut(0x1000 + source * 16 + idx)
}

impl Graph {
    /// Builds `init` on a fresh oracle graph.
    fn new(init: &Init) -> Self {
        let mut src = [ptr::null_mut(); SOURCES];
        // SAFETY: the oracle's constructors and the seam setters on its
        // live sources; the caller holds the graph lock.
        unsafe {
            c::oracle_slots_reset();
            for (i, s) in src.iter_mut().enumerate() {
                let kind = if i == OLD || i == NEW {
                    TRANSITION
                } else {
                    INPUT
                };
                *s = c::oracle_slots_source_new(kind, NAMES[i].as_ptr());
                assert!(!s.is_null());
                for idx in 0..2 {
                    seam::obs_source_transition_seam_set_texrender(*s, idx, texrender(i, idx));
                }
            }
            c::oracle_slots_refuse(src[NEW], src[SCENE]);
            for (t, slots) in init.slots.iter().enumerate() {
                for (idx, slot) in slots.iter().enumerate() {
                    let Some(child) = slot.source else { continue };
                    seam::obs_source_transition_seam_set_sources(src[t], idx, src[child]);
                    seam::obs_source_transition_seam_set_source_active(src[t], idx, slot.active);
                    let refs = c::oracle_slots_refs(src[child]);
                    c::oracle_slots_set_refs(src[child], refs + 1);
                    if slot.active {
                        let links = c::oracle_slots_link(src[t], src[child]);
                        c::oracle_slots_set_link(src[t], src[child], links + 1);
                    }
                }
            }
            for t in [OLD, NEW] {
                let s = src[t];
                seam::obs_source_transition_seam_set_transitioning_video(s, init.video[t]);
                seam::obs_source_transition_seam_set_transitioning_audio(s, init.audio[t]);
                seam::obs_source_transition_seam_set_start_time(s, init.start_time[t]);
                seam::obs_source_transition_seam_set_duration(s, init.duration[t]);
                seam::obs_source_transition_seam_set_mode(s, init.mode[t]);
                seam::obs_source_transition_seam_set_manual_val(s, init.manual[t].0);
                seam::obs_source_transition_seam_set_manual_target(s, init.manual[t].1);
                seam::obs_source_transition_seam_set_manual_torque(s, 3.0);
            }
            if let Some(d) = init.dying {
                c::oracle_slots_set_refs(src[d], 0);
            }
        }
        Self { src }
    }

    fn id(&self, p: *mut c_void) -> Option<usize> {
        self.src.iter().position(|s| *s == p)
    }
}

/// A source's running flags, clock and manual state (the floats as bits).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Running {
    video: bool,
    audio: bool,
    start_time: u64,
    duration: u64,
    mode: c_int,
    manual_val: u32,
    manual_target: u32,
    manual_torque: u32,
}

/// What a graph looks like after a call.
#[derive(Debug, Clone, PartialEq)]
struct Snapshot {
    slots: Vec<[Slot; 2]>,
    flags: Vec<Running>,
    texrender: Vec<[usize; 2]>,
    /// `(transition_mutex free, transition_tex_mutex free)` per source.
    locks: Vec<(bool, bool)>,
    refs: Vec<i64>,
    links: Vec<i64>,
    calls: Vec<(c_int, Option<usize>, Option<usize>, c_int)>,
    enumerated: Vec<(Option<usize>, Option<usize>, bool)>,
}

/// Whether a mutex is free, by a trylock (and its unlock when it got it).
/// A mutex this thread holds reads as taken.
unsafe fn free(
    trylock: unsafe extern "C" fn(*mut c_void) -> c_int,
    unlock: unsafe extern "C" fn(*mut c_void) -> c_int,
    s: *mut c_void,
) -> bool {
    // SAFETY: a live test source's initialized mutex.
    unsafe {
        let got = trylock(s) == 0;
        if got {
            unlock(s);
        }
        got
    }
}

fn snapshot(g: &Graph, enumerated: &[(*mut c_void, *mut c_void, bool)]) -> Snapshot {
    let id = |p: *mut c_void| {
        if p.is_null() {
            None
        } else {
            Some(g.id(p).unwrap_or(99))
        }
    };
    // SAFETY: the seam getters, the trylocks and the oracle's readers on the
    // graph's live sources, under the graph lock.
    unsafe {
        let mut s = Snapshot {
            slots: Vec::new(),
            flags: Vec::new(),
            texrender: Vec::new(),
            locks: Vec::new(),
            refs: Vec::new(),
            links: Vec::new(),
            calls: Vec::new(),
            enumerated: enumerated
                .iter()
                .map(|&(p, ch, locked)| (id(p), id(ch), locked))
                .collect(),
        };
        for &p in &g.src {
            s.slots.push([0, 1].map(|idx| Slot {
                source: id(seam::obs_source_transition_seam_get_sources(p, idx)),
                active: seam::obs_source_transition_seam_get_source_active(p, idx),
            }));
            s.flags.push(Running {
                video: seam::obs_source_transition_seam_get_transitioning_video(p),
                audio: seam::obs_source_transition_seam_get_transitioning_audio(p),
                start_time: seam::obs_source_transition_seam_get_start_time(p),
                duration: seam::obs_source_transition_seam_get_duration(p),
                mode: seam::obs_source_transition_seam_get_mode(p),
                manual_val: seam::obs_source_transition_seam_get_manual_val(p).to_bits(),
                manual_target: seam::obs_source_transition_seam_get_manual_target(p).to_bits(),
                manual_torque: seam::obs_source_transition_seam_get_manual_torque(p).to_bits(),
            });
            s.texrender.push(
                [0, 1].map(|idx| seam::obs_source_transition_seam_get_texrender(p, idx).addr()),
            );
            s.locks.push((
                free(
                    seam::obs_source_transition_seam_trylock,
                    seam::obs_source_transition_seam_unlock,
                    p,
                ),
                free(
                    seam::obs_source_transition_seam_trylock_textures,
                    seam::obs_source_transition_seam_unlock_textures,
                    p,
                ),
            ));
            s.refs.push(c::oracle_slots_refs(p));
            for &ch in &g.src {
                s.links.push(c::oracle_slots_link(p, ch));
            }
        }
        for i in 0..c::oracle_slots_traced() {
            let (mut kind, mut a, mut b, mut result) = (0, ptr::null_mut(), ptr::null_mut(), 0);
            c::oracle_slots_event(i, &raw mut kind, &raw mut a, &raw mut b, &raw mut result);
            s.calls.push((kind, id(a), id(b), result));
        }
        s
    }
}

/// The enum callback: records the parent, the child and whether the
/// parent's transition lock was held.
unsafe extern "C" fn record(parent: *mut c_void, child: *mut c_void, param: *mut c_void) {
    // SAFETY: param is the caller's record, parent a live test source.
    unsafe {
        let seen = &mut *param.cast::<Vec<(*mut c_void, *mut c_void, bool)>>();
        let locked = !free(
            seam::obs_source_transition_seam_trylock,
            seam::obs_source_transition_seam_unlock,
            parent,
        );
        seen.push((parent, child, locked));
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    C,
    Rust,
}

/// Runs `ops` on a graph built from `init` on one side; a snapshot after
/// every call, and between each swap's begin and end.
fn run_side(side: Side, init: &Init, ops: &[Op]) -> Vec<Snapshot> {
    let g = Graph::new(init);
    let mut out = Vec::new();
    let mut seen: Vec<(*mut c_void, *mut c_void, bool)> = Vec::new();
    for &op in ops {
        seen.clear();
        // SAFETY: the graph's live sources (or NULL for the clear, which
        // checks it), and a callback that takes `seen`.
        unsafe {
            match (op, side) {
                (Op::Clear(t), Side::C) => {
                    c::oracle_obs_transition_clear(t.map_or(ptr::null_mut(), |t| g.src[t]));
                }
                (Op::Clear(t), Side::Rust) => {
                    rs::clear::<Oracle>(t.map_or(ptr::null_mut(), |t| g.src[t]));
                }
                (Op::Enum(t), Side::C) => {
                    c::oracle_obs_transition_enum_sources(g.src[t], record, (&raw mut seen).cast())
                }
                (Op::Enum(t), Side::Rust) => {
                    rs::enum_sources::<Oracle>(g.src[t], record, (&raw mut seen).cast());
                }
                (Op::Swap(d, s), Side::C) => {
                    c::oracle_obs_transition_swap_begin(g.src[d], g.src[s]);
                    out.push(snapshot(&g, &seen));
                    c::oracle_obs_transition_swap_end(g.src[d], g.src[s]);
                }
                (Op::Swap(d, s), Side::Rust) => {
                    rs::swap_begin::<Oracle>(g.src[d], g.src[s]);
                    out.push(snapshot(&g, &seen));
                    rs::swap_end::<Oracle>(g.src[d], g.src[s]);
                }
            }
        }
        out.push(snapshot(&g, &seen));
    }
    // SAFETY: frees the graph's sources, under the graph lock.
    unsafe { c::oracle_slots_reset() };
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

fn every_op() -> Vec<Op> {
    let mut ops = vec![Op::Clear(None)];
    for t in [OLD, NEW, NOT_TR] {
        ops.push(Op::Enum(t));
    }
    for d in [OLD, NEW, NOT_TR] {
        for s in [OLD, NEW, NOT_TR] {
            ops.push(Op::Swap(d, s));
        }
    }
    for t in [OLD, NEW, NOT_TR] {
        ops.push(Op::Clear(Some(t)));
    }
    ops
}

fn fixed_inits() -> Vec<Init> {
    let (a, b, ch) = (CHILD_A, CHILD_B, CHILD_C);
    let mut inits = vec![
        // The C test's running transition and an empty new one.
        Init::running([held(a, true), held(b, true)], [EMPTY; 2]),
        // The new one held something; the old one's slot B is empty.
        Init::running([held(a, true), EMPTY], [held(ch, true), held(b, true)]),
        // Inactive slots, and an inactive scene that contains the new one.
        Init::running(
            [held(a, false), held(SCENE, false)],
            [held(b, false), EMPTY],
        ),
        // The new transition, and a scene that contains it, active in the
        // old one's slots: both refused.
        Init::running([held(NEW, true), held(SCENE, true)], [held(a, true), EMPTY]),
        // The old one holds itself, inactive; the new one holds the old one.
        Init::running([held(OLD, false), held(a, true)], [held(OLD, true), EMPTY]),
        // The same child in both slots.
        Init::running(
            [held(a, true), held(a, true)],
            [held(a, true), held(a, false)],
        ),
        // A source that is not a transition holding children.
        Init {
            slots: [[EMPTY; 2], [EMPTY; 2], [held(a, true), held(b, false)]],
            ..Init::running([EMPTY; 2], [EMPTY; 2])
        },
    ];
    // A child going away, active and not.
    let mut dying = Init::running([held(a, true), held(b, false)], [EMPTY; 2]);
    dying.dying = Some(a);
    inits.push(dying.clone());
    dying.dying = Some(b);
    inits.push(dying);
    // A manual transition, and one running audio only.
    let mut manual = Init::running([held(a, true), held(b, true)], [held(ch, true), EMPTY]);
    manual.mode = [1, 0];
    manual.video = [false, true];
    manual.audio = [true, true];
    inits.push(manual);
    inits
}

#[test]
fn every_call_on_fixed_graphs() {
    for init in fixed_inits() {
        // Each call alone, then all of them in a row.
        for op in every_op() {
            run(&init, &[op]).unwrap_or_else(|e| panic!("{e}"));
        }
        run(&init, &every_op()).unwrap_or_else(|e| panic!("{e}"));
    }
}

#[test]
fn the_c_tests_swaps_both_ways() {
    let init = Init::running([held(CHILD_A, true), held(CHILD_B, true)], [EMPTY; 2]);
    let ops = [
        Op::Swap(NEW, OLD),
        Op::Enum(NEW),
        Op::Swap(OLD, NEW),
        Op::Enum(OLD),
        Op::Swap(NEW, OLD),
        Op::Clear(Some(NEW)),
        Op::Clear(Some(OLD)),
    ];
    run(&init, &ops).unwrap_or_else(|e| panic!("{e}"));
}

fn gen_slot() -> impl Strategy<Value = Slot> {
    (0..SOURCES + 2, any::<bool>()).prop_map(|(s, active)| Slot {
        // OLD, NEW, NOT_TR and the children; the two past the end are empty.
        source: (s < SOURCES).then_some(s),
        active,
    })
}

fn gen_op() -> impl Strategy<Value = Op> {
    let t = 0..3usize;
    prop_oneof![
        proptest::option::of(t.clone()).prop_map(Op::Clear),
        t.clone().prop_map(Op::Enum),
        (t.clone(), t).prop_map(|(d, s)| Op::Swap(d, s)),
    ]
}

prop_compose! {
    fn gen_init()(
        slots in proptest::array::uniform3(proptest::array::uniform2(gen_slot())),
        video in proptest::array::uniform2(any::<bool>()),
        audio in proptest::array::uniform2(any::<bool>()),
        start_time in proptest::array::uniform2(any::<u64>()),
        duration in proptest::array::uniform2(any::<u64>()),
        mode in proptest::array::uniform2(0..2),
        manual in proptest::array::uniform2((0.0f32..=1.0, 0.0f32..=1.0)),
        dying in proptest::option::of(CHILD_A..=SCENE),
    ) -> Init {
        Init { slots, video, audio, start_time, duration, mode, manual, dying }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases(256)))]

    /// Any graph and any calls: the same state, references, links, calls
    /// and enumerations after each.
    #[test]
    fn any_graph_and_calls(init in gen_init(), ops in proptest::collection::vec(gen_op(), 1..8)) {
        run(&init, &ops)?;
    }
}

// ---------------------------------------------------------------------------
// The recursion warning

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

/// A refused add logs one warning naming the destination and the child,
/// the same line from both sides; a swap that stores everything logs
/// nothing. Swaps the process-wide log handler, so it runs in a child
/// process.
#[test]
fn recursion_warning_logs() {
    if !crate::output::isolated(concat!(module_path!(), "::recursion_warning_logs")) {
        return;
    }
    let mut log = Capture::arm();
    for (refused, name) in [(NEW, "new transition"), (SCENE, "a scene")] {
        let init = Init::running([held(refused, true), EMPTY], [EMPTY; 2]);
        let mut lines = Vec::new();
        for side in [Side::C, Side::Rust] {
            let _g = graph_lock();
            run_side(side, &init, &[Op::Swap(NEW, OLD)]);
            lines.push(log.take());
        }
        let want = format!(
            "obs_transition_swap_begin: cannot give transition 'new transition' source '{name}' due to infinite recursion"
        );
        assert_eq!(lines[0], (1, LOG_WARNING, want), "the C's line");
        assert_eq!(lines[1], lines[0], "the Rust line");
    }
    let init = Init::running([held(CHILD_A, true), held(CHILD_B, false)], [EMPTY; 2]);
    for side in [Side::C, Side::Rust] {
        let _g = graph_lock();
        run_side(side, &init, &[Op::Swap(NEW, OLD), Op::Clear(Some(NEW))]);
        assert_eq!(log.take().0, 0);
    }
}
