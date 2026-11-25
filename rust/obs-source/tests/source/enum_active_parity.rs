//! Tier 3: the original C `obs_source_enum_active_sources()` and
//! `obs_source_enum_active_tree()` and the Rust shims, on noise-filled
//! real-header sources, each reaching the same recording stand-ins
//! (oracle/source_enum_active.c) for `obs_source_get_ref()`,
//! `obs_source_release()` and `obs_transition_enum_sources()`, the
//! production field adapters and a scripted `enum_active_sources`.
//! Compares every plugin call (with whether it got the caller's callback
//! and parameter), reference, transition enum and reported edge in order,
//! the log lines and every byte of the sources. The exported symbols are
//! the same shims over libobs's own calls. No intentional differences;
//! a NULL child reported to the tree walk (the C dereferences it) is left
//! out.
use core::ffi::{CStr, c_char, c_void};
use obs_source::ffi::source_enum_active::{
    Calls, EnumProc, enum_active_sources_with, enum_active_tree_with,
};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

const NODES: usize = 5;
const CHILDREN: usize = 3;
const EVENTS: usize = 64;

const TRANSITION: u8 = 1;
const CALLBACK: u8 = 2;
const DATA: u8 = 4;
const DYING: u8 = 8;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct Case {
    which: u8,
    root: i8,
    noise: u8,
    flags: [u8; NODES],
    children: [[i8; CHILDREN]; NODES],
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

type Enum = unsafe extern "C" fn(*mut c_void, EnumProc, *mut c_void);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

unsafe extern "C" {
    fn oracle_source_enum_active_originals(sources: *mut Option<Enum>, tree: *mut Option<Enum>);
    fn oracle_source_enum_active_run(
        sources: Enum,
        tree: Enum,
        get: GetLog,
        set: SetLog,
        case: *const Case,
        out: *mut Outcome,
    ) -> bool;
    fn oracle_enum_active_get_ref(source: *mut c_void) -> *mut c_void;
    fn oracle_enum_active_release(source: *mut c_void);
    fn oracle_enum_active_transition_enum(source: *mut c_void, cb: EnumProc, param: *mut c_void);
}

/// The oracle's stand-ins, as the Rust shims' calls.
#[derive(Default)]
struct Stand;

impl Calls for Stand {
    unsafe fn transition_enum(source: *mut c_void, callback: EnumProc, param: *mut c_void) {
        // SAFETY: the shim's contract, forwarded to the recording stand-in.
        unsafe { oracle_enum_active_transition_enum(source, callback, param) }
    }

    unsafe fn get_ref(source: *mut c_void) -> *mut c_void {
        // SAFETY: as above.
        unsafe { oracle_enum_active_get_ref(source) }
    }

    unsafe fn release(source: *mut c_void) {
        // SAFETY: as above.
        unsafe { oracle_enum_active_release(source) }
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
        unsafe { oracle_enum_active_get_ref(source) }
    }

    unsafe fn release(source: *mut c_void) {
        // SAFETY: as `Stand`.
        unsafe { oracle_enum_active_release(source) }
    }
}

/// A stand-in that never releases (mutant).
#[derive(Default)]
struct Leaks;

impl Calls for Leaks {
    unsafe fn transition_enum(source: *mut c_void, callback: EnumProc, param: *mut c_void) {
        // SAFETY: as `Stand`.
        unsafe { oracle_enum_active_transition_enum(source, callback, param) }
    }

    unsafe fn get_ref(source: *mut c_void) -> *mut c_void {
        // SAFETY: as `Stand`.
        unsafe { oracle_enum_active_get_ref(source) }
    }

    unsafe fn release(source: *mut c_void) {
        let _ = source;
    }
}

macro_rules! shims {
    ($sources:ident, $tree:ident, $calls:ty) => {
        unsafe extern "C" fn $sources(source: *mut c_void, cb: EnumProc, param: *mut c_void) {
            // SAFETY: the oracle's fixture keeps the sources alive.
            unsafe { enum_active_sources_with::<$calls>(source, cb, param) }
        }
        unsafe extern "C" fn $tree(source: *mut c_void, cb: EnumProc, param: *mut c_void) {
            // SAFETY: as above.
            unsafe { enum_active_tree_with::<$calls>(source, cb, param) }
        }
    };
}

shims!(rust_sources, rust_tree, Stand);
shims!(no_slots_sources, no_slots_tree, NoSlots);
shims!(leaks_sources, leaks_tree, Leaks);

/// Mutant: the tree walk reports the edge before the child's subtree.
unsafe extern "C" fn pre_order_tree(source: *mut c_void, cb: EnumProc, param: *mut c_void) {
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
        // SAFETY: a live child under the oracle's fixture.
        unsafe { enum_active_sources_with::<Stand>(child, walk, param) };
    }
    let mut data = Data { cb, param };
    // SAFETY: as `rust_sources`.
    unsafe { enum_active_sources_with::<Stand>(source, walk, (&raw mut data).cast()) }
}

/// Mutant: the direct call walks the tree.
unsafe extern "C" fn sources_as_tree(source: *mut c_void, cb: EnumProc, param: *mut c_void) {
    // SAFETY: as `rust_sources`.
    unsafe { enum_active_tree_with::<Stand>(source, cb, param) }
}

fn originals() -> (Enum, Enum) {
    let mut sources = None;
    let mut tree = None;
    // SAFETY: two writable slots for function pointers.
    unsafe { oracle_source_enum_active_originals(&raw mut sources, &raw mut tree) };
    (sources.unwrap(), tree.unwrap())
}

fn run(sources: Enum, tree: Enum, case: &Case) -> Outcome {
    let mut out = Outcome::default();
    // SAFETY: the isolated child owns the log handler; the case and the
    // outcome are valid for the call.
    let valid = unsafe {
        oracle_source_enum_active_run(
            sources,
            tree,
            base_get_log_handler,
            base_set_log_handler,
            case,
            &mut out,
        )
    };
    assert!(valid, "the oracle fixture or an adapter failed: {case:?}");
    out
}

fn check_with(sources: Enum, tree: Enum, case: &Case) -> Option<Outcome> {
    let (original_sources, original_tree) = originals();
    let original = run(original_sources, original_tree, case);
    let ported = run(sources, tree, case);
    (ported == original).then_some(original)
}

fn check(case: &Case) -> Outcome {
    check_with(rust_sources, rust_tree, case)
        .unwrap_or_else(|| panic!("the Rust shims disagree with the original C: {case:?}"))
}

/// Every node a full, data-holding node without children or slots.
fn base(which: u8, root: i8) -> Case {
    Case {
        which,
        root,
        flags: [CALLBACK | DATA; NODES],
        children: [[-1; CHILDREN]; NODES],
        slots: [[-1; 2]; NODES],
        ..Case::default()
    }
}

/// Node 0 lists 1 and 2, node 1 lists 3 and 4; node 2 is a transition with
/// slots 3 and 4.
fn shaped(which: u8, root: i8, flags: [u8; NODES]) -> Case {
    let mut c = base(which, root);
    c.flags = flags;
    c.children[0] = [1, 2, -1];
    c.children[1] = [3, 4, -1];
    c.children[2] = [4, -1, -1];
    c.slots[2] = [3, 4];
    c
}

const FLAG_SETS: [u8; 6] = [
    CALLBACK | DATA,
    DATA,
    CALLBACK,
    TRANSITION | DATA,
    TRANSITION | CALLBACK | DATA,
    CALLBACK | DATA | DYING,
];

fn grid() -> impl Iterator<Item = Case> {
    let mut cases = Vec::new();
    for which in 0..2u8 {
        for root in -1i8..3 {
            for &root_flags in &FLAG_SETS {
                for &inner in &FLAG_SETS {
                    let mut flags = [CALLBACK | DATA; NODES];
                    if root >= 0 {
                        flags[usize::try_from(root).unwrap()] = root_flags;
                    }
                    flags[2] = if root == 2 { root_flags } else { inner };
                    flags[1] = if root == 1 { root_flags } else { inner };
                    cases.push(shaped(which, root, flags));
                }
            }
        }
    }
    cases.into_iter()
}

#[test]
fn enum_active_matches_c() {
    if !crate::output::isolated(concat!(module_path!(), "::enum_active_matches_c")) {
        return;
    }
    pinned();
    grid_and_mutants();
    random_cases();
}

fn pinned() {
    // Event 1 is a plugin call (node, 3 when it got the caller's callback
    // and parameter), 2 get_ref, 3 release, 4 a transition enum, 5 an edge
    // reported to the caller (parent, child).
    let direct = check(&shaped(0, 0, [CALLBACK | DATA; NODES]));
    assert!(direct.untouched && direct.log_count == 0);
    assert_eq!(
        direct.events(),
        [[2, 0, 0], [1, 0, 3], [5, 0, 1], [5, 0, 2], [3, 0, 0]]
    );
    let tree = check(&shaped(1, 0, [CALLBACK | DATA; NODES]));
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
            [5, 0, 2],
            [3, 0, 0],
        ]
    );
    let mut flags = [CALLBACK | DATA; NODES];
    flags[2] = TRANSITION | DATA;
    let transition = check(&shaped(1, 0, flags));
    assert!(transition.events().contains(&[4, 2, 0]));
    for (which, function) in [
        (0, "obs_source_enum_active_sources"),
        (1, "obs_source_enum_active_tree"),
    ] {
        let null = check(&base(which, -1));
        assert!(null.events().is_empty());
        assert_eq!((null.log_count, null.log_level), (1, 400));
        assert_eq!(null.log(), format!("{function}: Null 'source' parameter"));
        let mut dying = base(which, 0);
        dying.flags[0] |= DYING;
        let dying = check(&dying);
        assert_eq!(dying.events(), [[2, 0, 0]]);
        assert_eq!(dying.log(), format!("{function}: Null 'source' parameter"));
        let mut bare = base(which, 0);
        bare.flags[0] = DATA;
        assert!(check(&bare).events().is_empty());
        bare.flags[0] = CALLBACK;
        assert!(check(&bare).events().is_empty());
    }
}

fn grid_and_mutants() {
    let mut count = 0usize;
    for c in grid() {
        let _ = check(&c);
        count += 1;
    }
    assert_eq!(count, 2 * 4 * FLAG_SETS.len() * FLAG_SETS.len());

    let mutants: [(&str, Enum, Enum); 4] = [
        ("transition slots dropped", no_slots_sources, no_slots_tree),
        ("never released", leaks_sources, leaks_tree),
        ("pre-order tree", rust_sources, pre_order_tree),
        ("direct call walks the tree", sources_as_tree, rust_tree),
    ];
    for (name, sources, tree) in mutants {
        assert!(
            grid().any(|c| check_with(sources, tree, &c).is_none()),
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
                (0u8..2, -1i8..5, any::<u8>()),
                [0u8..16, 0u8..16, 0u8..16, 0u8..16, 0u8..16],
                proptest::collection::vec(-1i8..5, NODES * CHILDREN),
                proptest::collection::vec(-1i8..5, NODES * 2),
            ),
            |((which, root, noise), flags, children, slots)| {
                let mut c = base(which, root);
                c.noise = noise;
                c.flags = flags;
                for (i, row) in c.children.iter_mut().enumerate() {
                    row.copy_from_slice(&children[i * CHILDREN..(i + 1) * CHILDREN]);
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
