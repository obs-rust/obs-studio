//! Tier 3: the original C `obs_source_enum_full_tree()` and
//! `obs_source_active_child_enum_full_tree()` and the Rust shims, on
//! noise-filled real-header sources, each reaching the same recording
//! stand-ins (oracle/source_enum_full_tree.c) for `obs_source_get_ref()`,
//! `obs_source_release()` and `obs_transition_enum_sources()`, the
//! production field adapters and scripted `enum_all_sources` and
//! `enum_active_sources`. Compares every plugin call (with whether it got
//! the caller's callback and parameter), reference, transition enum and
//! reported edge in order, the log lines and every byte of the sources. The
//! exported symbols are the same shims over libobs's own calls. No
//! intentional differences; a NULL child reported to the walk with a stop
//! source (the C dereferences it) is left out.
use core::ffi::{CStr, c_char, c_void};
use obs_source::ffi::source_enum_full_tree::{Calls, EnumProc, enum_full_tree_with};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

const NODES: usize = 5;
const CHILDREN: usize = 3;
const EVENTS: usize = 96;
const CHILD_NULL: i8 = -2;

const TRANSITION: u8 = 1;
const ALL: u8 = 2;
const ACTIVE: u8 = 4;
const DATA: u8 = 8;
const DYING: u8 = 16;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct Case {
    which: u8,
    root: i8,
    stop: i8,
    noise: u8,
    flags: [u8; NODES],
    all: [[i8; CHILDREN]; NODES],
    active: [[i8; CHILDREN]; NODES],
    slots: [[i8; 2]; NODES],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Outcome {
    events: u32,
    trace: [u64; 3 * EVENTS],
    log_count: u32,
    log_level: i32,
    log: [c_char; 160],
    untouched: bool,
}

impl Default for Outcome {
    fn default() -> Self {
        Self {
            events: 0,
            trace: [0; 3 * EVENTS],
            log_count: 0,
            log_level: 0,
            log: [0; 160],
            untouched: false,
        }
    }
}

impl Outcome {
    fn events(&self) -> Vec<[u64; 3]> {
        self.trace
            .chunks(3)
            .take(usize::try_from(self.events).unwrap().min(EVENTS))
            .map(|e| [e[0], e[1], e[2]])
            .collect()
    }

    fn log(&self) -> &str {
        // SAFETY: the oracle wrote a NUL-terminated string (or left zeros).
        unsafe { CStr::from_ptr(self.log.as_ptr()) }
            .to_str()
            .unwrap()
    }
}

type Walk = unsafe extern "C" fn(*mut c_void, EnumProc, *mut c_void);
type StopWalk = unsafe extern "C" fn(*mut c_void, EnumProc, *mut c_void, *mut c_void);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

unsafe extern "C" {
    fn oracle_source_enum_full_tree_originals(
        walk: *mut Option<Walk>,
        stop_walk: *mut Option<StopWalk>,
    );
    fn oracle_source_enum_full_tree_run(
        walk: Walk,
        stop_walk: StopWalk,
        get: GetLog,
        set: SetLog,
        case: *const Case,
        out: *mut Outcome,
    ) -> bool;
    fn oracle_full_tree_get_ref(source: *mut c_void) -> *mut c_void;
    fn oracle_full_tree_release(source: *mut c_void);
    fn oracle_full_tree_transition_enum(source: *mut c_void, cb: EnumProc, param: *mut c_void);
}

/// The oracle's stand-ins, as the Rust shims' calls.
#[derive(Default)]
struct Stand;

impl Calls for Stand {
    unsafe fn transition_enum(source: *mut c_void, callback: EnumProc, param: *mut c_void) {
        // SAFETY: the shim's contract, forwarded to the recording stand-in.
        unsafe { oracle_full_tree_transition_enum(source, callback, param) }
    }

    unsafe fn get_ref(source: *mut c_void) -> *mut c_void {
        // SAFETY: as above.
        unsafe { oracle_full_tree_get_ref(source) }
    }

    unsafe fn release(source: *mut c_void) {
        // SAFETY: as above.
        unsafe { oracle_full_tree_release(source) }
    }
}

/// A stand-in that never reports a transition's slots (mutant).
#[derive(Default)]
struct NoSlots;

impl Calls for NoSlots {
    unsafe fn transition_enum(source: *mut c_void, callback: EnumProc, param: *mut c_void) {
        let _ = (source, callback, param);
    }

    unsafe fn get_ref(source: *mut c_void) -> *mut c_void {
        // SAFETY: as `Stand`.
        unsafe { oracle_full_tree_get_ref(source) }
    }

    unsafe fn release(source: *mut c_void) {
        // SAFETY: as `Stand`.
        unsafe { oracle_full_tree_release(source) }
    }
}

/// A stand-in that never releases (mutant).
#[derive(Default)]
struct Leaks;

impl Calls for Leaks {
    unsafe fn transition_enum(source: *mut c_void, callback: EnumProc, param: *mut c_void) {
        // SAFETY: as `Stand`.
        unsafe { oracle_full_tree_transition_enum(source, callback, param) }
    }

    unsafe fn get_ref(source: *mut c_void) -> *mut c_void {
        // SAFETY: as `Stand`.
        unsafe { oracle_full_tree_get_ref(source) }
    }

    unsafe fn release(source: *mut c_void) {
        let _ = source;
    }
}

macro_rules! shims {
    ($walk:ident, $stop_walk:ident, $calls:ty) => {
        unsafe extern "C" fn $walk(source: *mut c_void, cb: EnumProc, param: *mut c_void) {
            // SAFETY: the oracle's fixture keeps the sources alive.
            unsafe { enum_full_tree_with::<$calls>(source, cb, param, core::ptr::null_mut()) }
        }
        unsafe extern "C" fn $stop_walk(
            source: *mut c_void,
            cb: EnumProc,
            param: *mut c_void,
            stop: *mut c_void,
        ) {
            // SAFETY: as above.
            unsafe { enum_full_tree_with::<$calls>(source, cb, param, stop) }
        }
    };
}

shims!(rust_walk, rust_stop_walk, Stand);
shims!(no_slots_walk, no_slots_stop_walk, NoSlots);
shims!(leaks_walk, leaks_stop_walk, Leaks);

/// Mutant: the stop source is ignored.
unsafe extern "C" fn stop_ignored(
    source: *mut c_void,
    cb: EnumProc,
    param: *mut c_void,
    _stop: *mut c_void,
) {
    // SAFETY: as `rust_walk`.
    unsafe { enum_full_tree_with::<Stand>(source, cb, param, core::ptr::null_mut()) }
}

/// Mutant: the walk reports each edge before the child's subtree.
unsafe extern "C" fn pre_order(source: *mut c_void, cb: EnumProc, param: *mut c_void) {
    #[repr(C)]
    struct Data {
        cb: EnumProc,
        param: *mut c_void,
    }
    unsafe extern "C" fn walk(parent: *mut c_void, child: *mut c_void, param: *mut c_void) {
        // SAFETY: `param` is the `Data` below, alive for the walk.
        let data = unsafe { &*param.cast::<Data>() };
        // SAFETY: the caller's callback with its parameter.
        unsafe { (data.cb)(parent, child, data.param) };
        if !child.is_null() {
            // SAFETY: a live child under the oracle's fixture; a nested
            // walk of its own children with this callback.
            unsafe { enum_full_tree_with::<Stand>(child, walk, param, core::ptr::null_mut()) };
        }
    }
    let mut data = Data { cb, param };
    // SAFETY: as `rust_walk`; the outer walk reports through `walk` only.
    unsafe {
        enum_full_tree_with::<Stand>(source, walk, (&raw mut data).cast(), core::ptr::null_mut());
    }
}

fn originals() -> (Walk, StopWalk) {
    let mut walk = None;
    let mut stop_walk = None;
    // SAFETY: two writable slots for function pointers.
    unsafe { oracle_source_enum_full_tree_originals(&raw mut walk, &raw mut stop_walk) };
    (walk.unwrap(), stop_walk.unwrap())
}

fn run(walk: Walk, stop_walk: StopWalk, case: &Case) -> Outcome {
    let mut out = Outcome::default();
    // SAFETY: the isolated child owns the log handler; the case and the
    // outcome are valid for the call.
    let valid = unsafe {
        oracle_source_enum_full_tree_run(
            walk,
            stop_walk,
            base_get_log_handler,
            base_set_log_handler,
            case,
            &mut out,
        )
    };
    assert!(valid, "the oracle fixture or an adapter failed: {case:?}");
    out
}

fn check_with(walk: Walk, stop_walk: StopWalk, case: &Case) -> Option<Outcome> {
    let (original_walk, original_stop_walk) = originals();
    let original = run(original_walk, original_stop_walk, case);
    let ported = run(walk, stop_walk, case);
    (ported == original).then_some(original)
}

fn check(case: &Case) -> Outcome {
    check_with(rust_walk, rust_stop_walk, case)
        .unwrap_or_else(|| panic!("the Rust shims disagree with the original C: {case:?}"))
}

/// Every node a data-holding node with both callbacks and no children.
fn base(which: u8, root: i8, stop: i8) -> Case {
    Case {
        which,
        root,
        stop,
        flags: [ALL | ACTIVE | DATA; NODES],
        all: [[-1; CHILDREN]; NODES],
        active: [[-1; CHILDREN]; NODES],
        slots: [[-1; 2]; NODES],
        ..Case::default()
    }
}

/// Node 0 lists 1 and 2 through enum_all_sources and 3 through
/// enum_active_sources; node 1 lists 3 and 4 the first way and 2 the
/// second; node 2 lists 4 (and NULL) and is a transition with slots 3 and 4
/// when its flags say so.
fn shaped(which: u8, root: i8, stop: i8, flags: [u8; NODES]) -> Case {
    let mut c = base(which, root, stop);
    c.flags = flags;
    c.all[0] = [1, 2, -1];
    c.active[0] = [3, -1, -1];
    c.all[1] = [3, 4, -1];
    c.active[1] = [2, -1, -1];
    c.all[2] = [4, CHILD_NULL, -1];
    c.active[2] = [3, -1, -1];
    c.slots[2] = [3, 4];
    c
}

const FLAG_SETS: [u8; 8] = [
    ALL | ACTIVE | DATA,
    ALL | DATA,
    ACTIVE | DATA,
    DATA,
    ALL | ACTIVE,
    TRANSITION | DATA,
    TRANSITION | ACTIVE,
    ALL | DATA | DYING,
];

fn grid() -> impl Iterator<Item = Case> {
    let mut cases = Vec::new();
    for (which, stops) in [(0u8, &[-1i8][..]), (1, &[-1, 1, 2, 3])] {
        for &stop in stops {
            for root in -1i8..3 {
                for &root_flags in &FLAG_SETS {
                    for &inner in &FLAG_SETS {
                        let mut flags = [ALL | ACTIVE | DATA; NODES];
                        flags[1] = if root == 1 { root_flags } else { inner };
                        flags[2] = if root == 2 { root_flags } else { inner };
                        if root == 0 {
                            flags[0] = root_flags;
                        }
                        cases.push(shaped(which, root, stop, flags));
                    }
                }
            }
        }
    }
    cases.into_iter()
}

#[test]
fn enum_full_tree_matches_c() {
    if !crate::output::isolated(concat!(module_path!(), "::enum_full_tree_matches_c")) {
        return;
    }
    pinned();
    grid_and_mutants();
    random_cases();
}

fn pinned() {
    // Event 1 is an enum_all_sources call and 6 an enum_active_sources call
    // (node, 3 when it got the caller's callback and parameter), 2 get_ref,
    // 3 release, 4 a transition enum, 5 an edge reported to the caller
    // (parent, child; 0xfe for NULL).
    let tree = check(&shaped(0, 0, -1, [ALL | ACTIVE | DATA; NODES]));
    assert!(tree.untouched && tree.log_count == 0);
    assert_eq!(
        tree.events(),
        [
            [2, 0, 0],
            [1, 0, 0],
            [1, 1, 0],
            [1, 3, 0],
            [5, 1, 3],
            [1, 4, 0],
            [5, 1, 4],
            [5, 0, 1],
            [1, 2, 0],
            [1, 4, 0],
            [5, 2, 4],
            [5, 2, 0xfe],
            [5, 0, 2],
            [3, 0, 0],
        ]
    );
    // Only enum_active_sources: the root falls back to it.
    let mut flags = [ALL | ACTIVE | DATA; NODES];
    flags[0] = ACTIVE | DATA;
    let active = check(&shaped(0, 0, -1, flags));
    assert_eq!(active.events()[1], [6, 0, 0]);
    // A transition child without data still reports its slots.
    let mut flags = [ALL | ACTIVE | DATA; NODES];
    flags[2] = TRANSITION | ACTIVE;
    let transition = check(&shaped(0, 0, -1, flags));
    assert!(transition.events().contains(&[4, 2, 0]));
    assert!(!transition.events().contains(&[6, 2, 0]));
    // The stop source is reported, never walked.
    let stopped = check(&shaped(1, 0, 1, [ALL | ACTIVE | DATA; NODES]));
    assert!(stopped.events().contains(&[5, 0, 1]));
    assert!(!stopped.events().contains(&[1, 1, 0]));
    // NULL, dying and bare roots.
    for which in 0..2 {
        let null = check(&base(which, -1, -1));
        assert!(null.events().is_empty());
        assert_eq!((null.log_count, null.log_level), (1, 400));
        assert_eq!(
            null.log(),
            "obs_source_enum_full_tree: Null 'source' parameter"
        );
        let mut dying = base(which, 0, -1);
        dying.flags[0] |= DYING;
        let dying = check(&dying);
        assert_eq!(dying.events(), [[2, 0, 0]]);
        assert_eq!(
            dying.log(),
            "obs_source_enum_full_tree: Null 'source' parameter"
        );
        let mut bare = base(which, 0, -1);
        bare.flags[0] = DATA;
        assert!(check(&bare).events().is_empty());
        bare.flags[0] = ALL | ACTIVE;
        let silent = check(&bare);
        assert!(silent.events().is_empty() && silent.log_count == 0);
    }
}

fn grid_and_mutants() {
    let mut count = 0usize;
    for c in grid() {
        let _ = check(&c);
        count += 1;
    }
    assert_eq!(count, 5 * 4 * FLAG_SETS.len() * FLAG_SETS.len());

    let mutants: [(&str, Walk, StopWalk); 4] = [
        (
            "transition slots dropped",
            no_slots_walk,
            no_slots_stop_walk,
        ),
        ("never released", leaks_walk, leaks_stop_walk),
        ("pre-order walk", pre_order, rust_stop_walk),
        ("stop source ignored", rust_walk, stop_ignored),
    ];
    for (name, walk, stop_walk) in mutants {
        assert!(
            grid().any(|c| check_with(walk, stop_walk, &c).is_none()),
            "the mutant '{name}' agreed with the original C"
        );
    }
}

fn random_cases() {
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(512),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(
            &(
                (0u8..2, -1i8..5, -1i8..5, any::<u8>()),
                [0u8..32, 0u8..32, 0u8..32, 0u8..32, 0u8..32],
                proptest::collection::vec(-2i8..5, NODES * CHILDREN),
                proptest::collection::vec(-2i8..5, NODES * CHILDREN),
                proptest::collection::vec(-1i8..5, NODES * 2),
            ),
            |((which, root, stop, noise), flags, all, active, slots)| {
                let mut c = base(which, root, stop);
                c.noise = noise;
                c.flags = flags;
                for (i, row) in c.all.iter_mut().enumerate() {
                    row.copy_from_slice(&all[i * CHILDREN..(i + 1) * CHILDREN]);
                }
                for (i, row) in c.active.iter_mut().enumerate() {
                    row.copy_from_slice(&active[i * CHILDREN..(i + 1) * CHILDREN]);
                }
                for (i, row) in c.slots.iter_mut().enumerate() {
                    row.copy_from_slice(&slots[i * 2..(i + 1) * 2]);
                }
                let out = check(&c);
                prop_assert!(out.untouched);
                Ok(())
            },
        )
        .unwrap();
}
