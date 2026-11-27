//! Tier 3: the original C `obs_source_activate()` and
//! `obs_source_deactivate()` (with their four static tree callbacks) and the
//! Rust exports, on noise-filled real-header sources (oracle/
//! source_activate.c). Both reach the same scripted
//! `obs_source_enum_active_tree()`, which records its root and every
//! node's counters when it begins and reports a case's edges to the
//! callback it was given; the Rust side reaches the production counter
//! adapters. Compares every walk in order, the counters afterwards, the log
//! lines and every other byte of the sources, over every call, source
//! (NULL included), view type (MAIN, AUX and others), root counter edge and
//! edge set, plus a proptest; and rejects mutants. No intentional
//! differences. Excluded: a NULL child, which the real walk can't report
//! (it reads each child's type first); the original would dereference it.
use core::ffi::{CStr, c_char, c_long, c_void};
use core::ptr::NonNull;
use obs_source::activate::{AUX_VIEW, Counter, Counters, MAIN_VIEW, Tree};
use obs_source::ffi::source_activate::{Libobs, obs_source_activate, obs_source_deactivate};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

const NODES: usize = 4;
const EDGES: usize = 8;
const EVENTS: usize = 8;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct Case {
    which: u8,
    source: i8,
    noise: u8,
    view_type: u32,
    show: [c_long; NODES],
    active: [c_long; NODES],
    edges: u8,
    edge_parent: [i8; EDGES],
    edge_child: [i8; EDGES],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Outcome {
    events: u32,
    trace: [[i64; 1 + 2 * NODES]; EVENTS],
    show: [c_long; NODES],
    active: [c_long; NODES],
    log_count: u32,
    log_level: i32,
    log: [c_char; 128],
    untouched: bool,
}

impl Default for Outcome {
    fn default() -> Self {
        Self {
            events: 0,
            trace: [[0; 1 + 2 * NODES]; EVENTS],
            show: [0; NODES],
            active: [0; NODES],
            log_count: 0,
            log_level: 0,
            log: [0; 128],
            untouched: false,
        }
    }
}

impl Outcome {
    fn log(&self) -> &str {
        // SAFETY: the oracle wrote a NUL-terminated string (or left zeros).
        unsafe { CStr::from_ptr(self.log.as_ptr()) }
            .to_str()
            .unwrap()
    }

    /// The walks' roots, in order.
    fn roots(&self) -> Vec<i64> {
        self.trace
            .iter()
            .take(usize::try_from(self.events).unwrap())
            .map(|e| e[0])
            .collect()
    }
}

type Call = unsafe extern "C" fn(*mut c_void, u32);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

unsafe extern "C" {
    fn oracle_source_activate_originals(activate: *mut Option<Call>, deactivate: *mut Option<Call>);
    fn oracle_source_activate_run(
        activate: Call,
        deactivate: Call,
        get: GetLog,
        set: SetLog,
        case: *const Case,
        out: *mut Outcome,
    ) -> bool;
}

fn originals() -> (Call, Call) {
    let mut activate = None;
    let mut deactivate = None;
    // SAFETY: two writable slots for function pointers.
    unsafe { oracle_source_activate_originals(&raw mut activate, &raw mut deactivate) };
    (activate.unwrap(), deactivate.unwrap())
}

/// Runs one side; panics when the oracle could not build the fixture or an
/// adapter does not reach its field.
fn run(activate: Call, deactivate: Call, case: &Case) -> Outcome {
    let mut out = Outcome::default();
    // SAFETY: the isolated child owns the log handler; the case and the
    // outcome are valid for the call.
    let valid = unsafe {
        oracle_source_activate_run(
            activate,
            deactivate,
            base_get_log_handler,
            base_set_log_handler,
            case,
            &mut out,
        )
    };
    assert!(valid, "the oracle fixture or an adapter failed: {case:?}");
    out
}

/// The original's outcome, when the given calls agree with it.
fn check_with(activate: Call, deactivate: Call, case: &Case) -> Option<Outcome> {
    let (original_activate, original_deactivate) = originals();
    let original = run(original_activate, original_deactivate, case);
    let ported = run(activate, deactivate, case);
    (ported == original).then_some(original)
}

fn check(case: &Case) -> Outcome {
    check_with(obs_source_activate, obs_source_deactivate, case)
        .unwrap_or_else(|| panic!("the Rust exports disagree with the original C: {case:?}"))
}

fn case(which: u8, source: i8, view_type: u32, root: [c_long; 2], edges: &[(i8, i8)]) -> Case {
    let mut case = Case {
        which,
        source,
        noise: 0xa5,
        view_type,
        // Every node's counters differ, so a step on the wrong one shows.
        show: [11, 111, 211, 311],
        active: [13, 113, 213, 313],
        edges: u8::try_from(edges.len()).unwrap(),
        ..Case::default()
    };
    if let Ok(i) = usize::try_from(source) {
        case.show[i] = root[0];
        case.active[i] = root[1];
    }
    for (i, (p, c)) in edges.iter().enumerate() {
        case.edge_parent[i] = *p;
        case.edge_child[i] = *c;
    }
    case
}

/// Edge sets the walk reports from node 0: none, the C test's tree
/// (post-order: c under b, b and d under a), the root itself, a repeated
/// child, and a NULL parent.
const EDGE_SETS: [&[(i8, i8)]; 5] = [
    &[],
    &[(1, 2), (0, 1), (0, 3)],
    &[(0, 0)],
    &[(0, 1), (0, 1), (2, 3)],
    &[(-1, 2)],
];

// --- mutants ---------------------------------------------------------------

/// A hand-written port with one change, on the real adapters and walk.
#[derive(Clone, Copy)]
enum Change {
    /// NULL logs nothing.
    SilentNull,
    /// NULL is logged under the other entry's name.
    SwapNullNames,
    /// MAIN_VIEW and AUX_VIEW swapped.
    SwapViews,
    /// The active step comes before the show step.
    ActiveFirst,
    /// Each walk runs the opposite callback (show for hide, and back).
    OppositeWalk,
    /// The show walk runs the active callback.
    WrongCounterWalk,
    /// Deactivation steps without checking the root's counter.
    NoFloorCheck,
    /// Deactivation checks `>= 0` instead of `> 0`.
    FloorAtZero,
    /// MAIN deactivation skips the active step when the show step ran.
    ActiveOnlyWhenHidden,
    /// The walk runs before the root's own step.
    WalkFirst,
    /// The walk is skipped.
    NoWalk,
}

fn opposite(tree: Tree) -> Tree {
    match tree {
        Tree::Show => Tree::Hide,
        Tree::Hide => Tree::Show,
        Tree::Activate => Tree::Deactivate,
        Tree::Deactivate => Tree::Activate,
    }
}

fn step(change: Change, source: NonNull<c_void>, counter: Counter, tree: Tree, raise: bool) {
    let tree = match change {
        Change::OppositeWalk => opposite(tree),
        Change::WrongCounterWalk if tree == Tree::Show => Tree::Activate,
        _ => tree,
    };
    let mut libobs = Libobs;
    let own = |libobs: &mut Libobs| {
        if raise {
            libobs.inc(source, counter);
        } else {
            libobs.dec(source, counter);
        }
    };
    match change {
        Change::WalkFirst => {
            libobs.walk(source, tree);
            own(&mut libobs);
        }
        Change::NoWalk => own(&mut libobs),
        _ => {
            own(&mut libobs);
            libobs.walk(source, tree);
        }
    }
}

fn is_main(change: Change, view_type: u32) -> bool {
    match change {
        Change::SwapViews => view_type != MAIN_VIEW,
        _ => view_type == MAIN_VIEW,
    }
}

fn null(change: Change, function: &'static CStr, other: &'static CStr) {
    match change {
        Change::SilentNull => {}
        Change::SwapNullNames => Libobs.log_null(other),
        _ => Libobs.log_null(function),
    }
}

fn mutant_activate(change: Change, source: *mut c_void, view_type: u32) {
    let Some(source) = NonNull::new(source) else {
        null(change, c"obs_source_activate", c"obs_source_deactivate");
        return;
    };
    let main = is_main(change, view_type);
    if main && matches!(change, Change::ActiveFirst) {
        step(change, source, Counter::Activate, Tree::Activate, true);
    }
    step(change, source, Counter::Show, Tree::Show, true);
    if main && !matches!(change, Change::ActiveFirst) {
        step(change, source, Counter::Activate, Tree::Activate, true);
    }
}

fn mutant_deactivate(change: Change, source: *mut c_void, view_type: u32) {
    let Some(source) = NonNull::new(source) else {
        null(change, c"obs_source_deactivate", c"obs_source_activate");
        return;
    };
    let mut libobs = Libobs;
    let above = |libobs: &mut Libobs, counter| match change {
        Change::NoFloorCheck => true,
        Change::FloorAtZero => libobs.load(source, counter) >= 0,
        _ => libobs.load(source, counter) > 0,
    };
    let main = is_main(change, view_type);
    if main && matches!(change, Change::ActiveFirst) && above(&mut libobs, Counter::Activate) {
        step(change, source, Counter::Activate, Tree::Deactivate, false);
    }
    let mut hid = false;
    if above(&mut libobs, Counter::Show) {
        step(change, source, Counter::Show, Tree::Hide, false);
        hid = true;
    }
    if main
        && !matches!(change, Change::ActiveFirst)
        && !(hid && matches!(change, Change::ActiveOnlyWhenHidden))
        && above(&mut libobs, Counter::Activate)
    {
        step(change, source, Counter::Activate, Tree::Deactivate, false);
    }
}

macro_rules! mutant {
    ($activate:ident, $deactivate:ident, $change:expr) => {
        unsafe extern "C" fn $activate(source: *mut c_void, view_type: u32) {
            mutant_activate($change, source, view_type);
        }
        unsafe extern "C" fn $deactivate(source: *mut c_void, view_type: u32) {
            mutant_deactivate($change, source, view_type);
        }
    };
}

mutant!(a_silent, d_silent, Change::SilentNull);
mutant!(a_swap_null, d_swap_null, Change::SwapNullNames);
mutant!(a_swap_views, d_swap_views, Change::SwapViews);
mutant!(a_active_first, d_active_first, Change::ActiveFirst);
mutant!(a_opposite, d_opposite, Change::OppositeWalk);
mutant!(a_wrong_counter, d_wrong_counter, Change::WrongCounterWalk);
mutant!(a_no_floor, d_no_floor, Change::NoFloorCheck);
mutant!(a_floor_zero, d_floor_zero, Change::FloorAtZero);
mutant!(a_only_hidden, d_only_hidden, Change::ActiveOnlyWhenHidden);
mutant!(a_walk_first, d_walk_first, Change::WalkFirst);
mutant!(a_no_walk, d_no_walk, Change::NoWalk);

// --- the checks ------------------------------------------------------------

fn pinned() {
    // NULL: one LOG_DEBUG line naming the entry, no walk, nothing written.
    for (which, function) in [(0, "obs_source_activate"), (1, "obs_source_deactivate")] {
        for view_type in [MAIN_VIEW, AUX_VIEW] {
            let out = check(&case(which, -1, view_type, [1, 1], EDGE_SETS[1]));
            assert_eq!((out.log_count, out.log_level), (1, 400));
            assert_eq!(out.log(), format!("{function}: Null 'source' parameter"));
            assert_eq!(out.events, 0);
            assert!(out.untouched);
            assert_eq!(out.show, [11, 111, 211, 311]);
        }
    }

    // MAIN activate on the C test's tree: two walks, every counter +1.
    let out = check(&case(0, 0, MAIN_VIEW, [0, 0], EDGE_SETS[1]));
    assert_eq!(out.roots(), [0, 0]);
    assert_eq!(out.trace[0][1..5], [1, 111, 211, 311]);
    assert_eq!(out.trace[1][1..5], [1, 112, 212, 312]);
    assert_eq!(out.trace[1][5..9], [1, 113, 213, 313]);
    assert_eq!(out.show, [1, 112, 212, 312]);
    assert_eq!(out.active, [1, 114, 214, 314]);
    assert_eq!(out.log_count, 0);
    assert!(out.untouched);

    // AUX activate: one walk, activate_refs untouched.
    let out = check(&case(0, 0, AUX_VIEW, [0, 0], EDGE_SETS[1]));
    assert_eq!(out.roots(), [0]);
    assert_eq!(out.active, [0, 113, 213, 313]);

    // Deactivate at zero: no walk, nothing moves.
    let out = check(&case(1, 0, MAIN_VIEW, [0, 0], EDGE_SETS[1]));
    assert_eq!(out.events, 0);
    assert_eq!(out.show, [0, 111, 211, 311]);
    assert_eq!(out.active, [0, 113, 213, 313]);

    // Hidden but active: only the active walk.
    let out = check(&case(1, 0, MAIN_VIEW, [0, 1], EDGE_SETS[1]));
    assert_eq!(out.roots(), [0]);
    assert_eq!(out.active, [0, 112, 212, 312]);

    // A repeated child moves twice; the root as its own child once more.
    let out = check(&case(0, 0, AUX_VIEW, [0, 0], EDGE_SETS[3]));
    assert_eq!(out.show, [1, 113, 211, 312]);
    let out = check(&case(0, 0, AUX_VIEW, [0, 0], EDGE_SETS[2]));
    assert_eq!(out.show[0], 2);
}

fn grid() -> impl Iterator<Item = Case> {
    let mut cases = Vec::new();
    for which in 0..2u8 {
        for source in [-1i8, 0, 2] {
            for view_type in [0, MAIN_VIEW, AUX_VIEW, 3, u32::MAX] {
                for show in [-2, -1, 0, 1, 2] {
                    for active in [-1, 0, 1, 3] {
                        for edges in EDGE_SETS {
                            cases.push(case(which, source, view_type, [show, active], edges));
                        }
                    }
                }
            }
        }
    }
    cases.into_iter()
}

fn grid_and_mutants() {
    let mut count = 0usize;
    for c in grid() {
        let out = check(&c);
        assert!(out.untouched);
        count += 1;
    }
    assert_eq!(count, 2 * 3 * 5 * 5 * 4 * EDGE_SETS.len());

    let mutants: [(&str, Call, Call); 11] = [
        ("silent NULL", a_silent, d_silent),
        ("NULL names swapped", a_swap_null, d_swap_null),
        ("views swapped", a_swap_views, d_swap_views),
        ("active step first", a_active_first, d_active_first),
        ("opposite walk", a_opposite, d_opposite),
        (
            "show walk runs the active callback",
            a_wrong_counter,
            d_wrong_counter,
        ),
        ("no floor check", a_no_floor, d_no_floor),
        ("floor at zero", a_floor_zero, d_floor_zero),
        ("active step only when hidden", a_only_hidden, d_only_hidden),
        ("walk before the root's step", a_walk_first, d_walk_first),
        ("no walk", a_no_walk, d_no_walk),
    ];
    for (name, activate, deactivate) in mutants {
        assert!(
            grid().any(|c| check_with(activate, deactivate, &c).is_none()),
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
    // Children are always nodes: the real walk never reports NULL.
    let edge = (-1i8..4, 0i8..4);
    runner
        .run(
            &(
                (0u8..2, -1i8..4, any::<u8>()),
                prop_oneof![Just(MAIN_VIEW), Just(AUX_VIEW), any::<u32>()],
                (
                    proptest::array::uniform4(-3 as c_long..4),
                    proptest::array::uniform4(-3 as c_long..4),
                ),
                proptest::collection::vec(edge, 0..=EDGES),
            ),
            |((which, source, noise), view_type, (show, active), edges)| {
                let mut c = case(which, source, view_type, [0, 0], &edges);
                c.noise = noise;
                c.show = show;
                c.active = active;
                let out = check(&c);
                prop_assert!(out.untouched);
                prop_assert!(out.events <= 2);
                Ok(())
            },
        )
        .unwrap();
}

#[test]
fn activate_matches_c() {
    if !crate::output::isolated(concat!(module_path!(), "::activate_matches_c")) {
        return;
    }
    pinned();
    grid_and_mutants();
    random_cases();
}
