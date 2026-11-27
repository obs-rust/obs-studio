//! Tier 3: the original C `obs_source_add_active_child()` and
//! `obs_source_remove_active_child()` and the Rust shims, on noise-filled
//! real-header sources, each reaching the same stand-ins (oracle/
//! source_active_child.c): a scripted full-tree walk reporting a case's
//! edges, the production counter adapters, and the tracing
//! `obs_source_activate()` / `obs_source_deactivate()` of oracle/view.c.
//! The walk is Rust since #5833 (its parity is enum_full_tree_parity), so
//! the Rust side runs [`add_with`] / [`remove_with`] over [`Scripted`]: the
//! production [`Libobs`] with the scripted walk in place of the real one.
//! Compares every walk (root, stop source), activation and deactivation
//! (source, view type) in order, the result, the log lines and every other
//! byte of the sources. No intentional differences or excluded inputs.
use core::ffi::{CStr, c_char, c_long, c_void};
use core::ptr::NonNull;
use obs_source::active_child::{self, AUX_VIEW, Libobs as _, MAIN_VIEW, names, view_type};
use obs_source::ffi::source_active_child::{Libobs, add_with, remove_with};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

const EDGES: usize = 8;
const EVENTS: usize = 24;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct Case {
    which: u8,
    parent: i8,
    child: i8,
    noise: u8,
    show_refs: c_long,
    activate_refs: c_long,
    edges: u8,
    edge_parent: [i8; EDGES],
    edge_child: [i8; EDGES],
    clobber: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Outcome {
    events: u32,
    trace: [u64; 3 * EVENTS],
    returned: bool,
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
            returned: false,
            log_count: 0,
            log_level: 0,
            log: [0; 160],
            untouched: false,
        }
    }
}

impl Outcome {
    /// The events as `(kind, a, b)` triples.
    fn events(&self) -> Vec<[u64; 3]> {
        self.trace
            .chunks(3)
            .take(usize::try_from(self.events).unwrap())
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

type Add = unsafe extern "C" fn(*mut c_void, *mut c_void) -> bool;
type Remove = unsafe extern "C" fn(*mut c_void, *mut c_void);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

unsafe extern "C" {
    fn oracle_source_active_child_originals(add: *mut Option<Add>, remove: *mut Option<Remove>);
    fn oracle_source_active_child_run(
        add: Add,
        remove: Remove,
        get: GetLog,
        set: SetLog,
        case: *const Case,
        out: *mut Outcome,
    ) -> bool;
}

fn originals() -> (Add, Remove) {
    let mut add = None;
    let mut remove = None;
    // SAFETY: two writable slots for function pointers.
    unsafe { oracle_source_active_child_originals(&raw mut add, &raw mut remove) };
    (add.unwrap(), remove.unwrap())
}

/// Runs one side; panics when the oracle could not build the fixture or an
/// adapter does not reach its field.
fn run(add: Add, remove: Remove, case: &Case) -> Outcome {
    let mut out = Outcome::default();
    // SAFETY: the isolated child owns the log handler and the view trace
    // hook; the case and the outcome are valid for the call.
    let valid = unsafe {
        oracle_source_active_child_run(
            add,
            remove,
            base_get_log_handler,
            base_set_log_handler,
            case,
            &mut out,
        )
    };
    assert!(valid, "the oracle fixture or an adapter failed: {case:?}");
    out
}

/// The original's outcome, after checking the given calls agree with it.
fn check_with(add: Add, remove: Remove, case: &Case) -> Option<Outcome> {
    let (original_add, original_remove) = originals();
    let original = run(original_add, original_remove, case);
    let ported = run(add, remove, case);
    (ported == original).then_some(original)
}

fn check(case: &Case) -> Outcome {
    check_with(rust_add, rust_remove, case)
        .unwrap_or_else(|| panic!("the Rust exports disagree with the original C: {case:?}"))
}

fn case(which: u8, parent: i8, child: i8, refs: [c_long; 2], edges: &[(i8, i8)]) -> Case {
    let mut case = Case {
        which,
        parent,
        child,
        noise: 0xa5,
        show_refs: refs[0],
        activate_refs: refs[1],
        edges: u8::try_from(edges.len()).unwrap(),
        ..Case::default()
    };
    for (i, (p, c)) in edges.iter().enumerate() {
        case.edge_parent[i] = *p;
        case.edge_child[i] = *c;
    }
    case
}

// --- the Rust side ----------------------------------------------------------

unsafe extern "C" {
    fn oracle_active_child_enum_full_tree(
        source: *mut c_void,
        enum_callback: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void),
        param: *mut c_void,
        stop_source: *mut c_void,
    );
}

/// The visitor the scripted walk reports to, as its `param`.
type ScriptVisit<'a> = &'a mut dyn FnMut(Option<NonNull<c_void>>, Option<NonNull<c_void>>);

unsafe extern "C" fn scripted_edge(parent: *mut c_void, child: *mut c_void, param: *mut c_void) {
    // SAFETY: `param` is the `ScriptVisit` `Scripted::enum_full_tree` keeps
    // alive until the walk returns.
    let visit = unsafe { &mut *param.cast::<ScriptVisit<'_>>() };
    visit(NonNull::new(parent), NonNull::new(child));
}

/// The production calls with the oracle's scripted walk.
struct Scripted;

impl active_child::Libobs for Scripted {
    type Source = NonNull<c_void>;

    fn enum_full_tree(&mut self, root: Self::Source, stop: Self::Source, mut visit: Visit<'_>) {
        let param: *mut ScriptVisit<'_> = &mut visit;
        // SAFETY: the scripted walk calls `scripted_edge` with `param` only
        // while it runs.
        unsafe {
            oracle_active_child_enum_full_tree(
                root.as_ptr(),
                scripted_edge,
                param.cast(),
                stop.as_ptr(),
            );
        }
    }
    fn show_refs(&mut self, source: Self::Source) -> c_long {
        Libobs.show_refs(source)
    }
    fn activate_refs(&mut self, source: Self::Source) -> c_long {
        Libobs.activate_refs(source)
    }
    fn activate(&mut self, source: Self::Source, view: u32) {
        Libobs.activate(source, view);
    }
    fn deactivate(&mut self, source: Self::Source, view: u32) {
        Libobs.deactivate(source, view);
    }
    fn log_null(&mut self, function: &'static CStr, parameter: &'static CStr) {
        Libobs.log_null(function, parameter);
    }
    fn warn_self(&mut self) {
        Libobs.warn_self();
    }
}

/// `obs_source_add_active_child()`'s shim over [`Scripted`].
unsafe extern "C" fn rust_add(parent: *mut c_void, child: *mut c_void) -> bool {
    add_with(&mut Scripted, parent, child)
}

/// `obs_source_remove_active_child()`'s shim over [`Scripted`].
unsafe extern "C" fn rust_remove(parent: *mut c_void, child: *mut c_void) {
    remove_with(&mut Scripted, parent, child);
}

// --- mutants of the shims -------------------------------------------------

type Visit<'a> = &'a mut dyn FnMut(Option<NonNull<c_void>>, Option<NonNull<c_void>>);

/// The real C calls, with one of them changed.
#[derive(Clone, Copy)]
enum Change {
    /// The walk's edges lose their parent.
    DropEdgeParent,
    /// The walk's edges lose their child.
    DropEdgeChild,
    /// The walk stops at the child instead of the parent.
    StopAtRoot,
    /// MAIN_VIEW and AUX_VIEW swapped.
    SwapViews,
    /// Activations become deactivations and back.
    SwapDirection,
    /// NULL logs nothing.
    SilentNull,
    /// NULL names the other parameter.
    SwapNullNames,
}

struct Mutant(Change);

impl active_child::Libobs for Mutant {
    type Source = NonNull<c_void>;

    fn enum_full_tree(&mut self, root: Self::Source, stop: Self::Source, visit: Visit<'_>) {
        match self.0 {
            Change::DropEdgeParent => {
                Scripted.enum_full_tree(root, stop, &mut |_, c| visit(None, c))
            }
            Change::DropEdgeChild => {
                Scripted.enum_full_tree(root, stop, &mut |p, _| visit(p, None))
            }
            Change::StopAtRoot => Scripted.enum_full_tree(root, root, visit),
            _ => Scripted.enum_full_tree(root, stop, visit),
        }
    }
    fn show_refs(&mut self, source: Self::Source) -> c_long {
        Scripted.show_refs(source)
    }
    fn activate_refs(&mut self, source: Self::Source) -> c_long {
        Scripted.activate_refs(source)
    }
    fn activate(&mut self, source: Self::Source, view: u32) {
        match self.0 {
            Change::SwapViews => Scripted.activate(source, swap(view)),
            Change::SwapDirection => Scripted.deactivate(source, view),
            _ => Scripted.activate(source, view),
        }
    }
    fn deactivate(&mut self, source: Self::Source, view: u32) {
        match self.0 {
            Change::SwapViews => Scripted.deactivate(source, swap(view)),
            Change::SwapDirection => Scripted.activate(source, view),
            _ => Scripted.deactivate(source, view),
        }
    }
    fn log_null(&mut self, function: &'static CStr, parameter: &'static CStr) {
        match self.0 {
            Change::SilentNull => {}
            Change::SwapNullNames => Scripted.log_null(
                function,
                if parameter == active_child::PARENT {
                    active_child::CHILD
                } else {
                    active_child::PARENT
                },
            ),
            _ => Scripted.log_null(function, parameter),
        }
    }
    fn warn_self(&mut self) {
        Scripted.warn_self();
    }
}

fn swap(view: u32) -> u32 {
    if view == MAIN_VIEW {
        AUX_VIEW
    } else {
        MAIN_VIEW
    }
}

macro_rules! mutant {
    ($add:ident, $remove:ident, $change:expr) => {
        unsafe extern "C" fn $add(parent: *mut c_void, child: *mut c_void) -> bool {
            add_with(&mut Mutant($change), parent, child)
        }
        unsafe extern "C" fn $remove(parent: *mut c_void, child: *mut c_void) {
            remove_with(&mut Mutant($change), parent, child);
        }
    };
}

mutant!(add_drop_parent, remove_drop_parent, Change::DropEdgeParent);
mutant!(add_drop_child, remove_drop_child, Change::DropEdgeChild);
mutant!(add_stop_root, remove_stop_root, Change::StopAtRoot);
mutant!(add_swap_views, remove_swap_views, Change::SwapViews);
mutant!(
    add_swap_direction,
    remove_swap_direction,
    Change::SwapDirection
);
mutant!(add_silent_null, remove_silent_null, Change::SilentNull);
mutant!(add_swap_null, remove_swap_null, Change::SwapNullNames);

/// Loads the parent's show count again before each activation.
unsafe extern "C" fn add_reloading(parent: *mut c_void, child: *mut c_void) -> bool {
    let (Some(p), Some(c)) = (NonNull::new(parent), NonNull::new(child)) else {
        // SAFETY: as the real export.
        return unsafe { rust_add(parent, child) };
    };
    if p == c {
        // SAFETY: as the real export.
        return unsafe { rust_add(parent, child) };
    }
    let mut exists = false;
    Scripted.enum_full_tree(c, p, &mut |a, b| exists |= names(a, b, p));
    if exists {
        return false;
    }
    let activate_refs = Scripted.activate_refs(p);
    let mut index: c_long = 0;
    while index < Scripted.show_refs(p) {
        Scripted.activate(c, view_type(index, activate_refs));
        index += 1;
    }
    true
}

/// No parent == child check.
unsafe extern "C" fn add_no_self_check(parent: *mut c_void, child: *mut c_void) -> bool {
    match (NonNull::new(parent), NonNull::new(child)) {
        (Some(p), Some(c)) if p == c => {
            let show = Scripted.show_refs(p);
            let active = Scripted.activate_refs(p);
            for index in 0..show {
                Scripted.activate(c, view_type(index, active));
            }
            true
        }
        // SAFETY: as the real export.
        _ => unsafe { rust_add(parent, child) },
    }
}

/// Remove walks the child's tree first, as add does.
unsafe extern "C" fn remove_walking(parent: *mut c_void, child: *mut c_void) {
    if let (Some(p), Some(c)) = (NonNull::new(parent), NonNull::new(child)) {
        Scripted.enum_full_tree(c, p, &mut |_, _| {});
    }
    // SAFETY: as the real export.
    unsafe { rust_remove(parent, child) }
}

// --- the grid --------------------------------------------------------------

const EDGE_SETS: [&[(i8, i8)]; 10] = [
    &[],
    &[(1, 2)],
    &[(1, 0)],
    &[(0, 2)],
    &[(2, 3), (3, 0)],
    &[(-1, 0)],
    &[(0, -1)],
    &[(-1, -1)],
    &[(1, 2), (2, 1)],
    &[(2, 1), (1, 3), (3, 2)],
];

fn grid() -> impl Iterator<Item = Case> {
    let mut cases = Vec::new();
    for which in 0..2u8 {
        for parent in [-1i8, 0] {
            for child in [-1i8, 0, 1] {
                for show in [-2, -1, 0, 1, 2, 3, 5] {
                    for active in [-1, 0, 1, 2, 3, 6] {
                        for edges in EDGE_SETS {
                            for clobber in [false, true] {
                                let mut c = case(which, parent, child, [show, active], edges);
                                c.clobber = clobber;
                                cases.push(c);
                            }
                        }
                    }
                }
            }
        }
    }
    cases.into_iter()
}

#[test]
fn active_child_matches_c() {
    if !crate::output::isolated(concat!(module_path!(), "::active_child_matches_c")) {
        return;
    }
    pinned();
    grid_and_mutants();
    random_cases();
}

fn pinned() {
    // Pinned answers: the shared result itself, so a change on both sides
    // (or in the oracle) still fails. MAIN_VIEW is 1, AUX_VIEW 2; event 1
    // is a walk (root, stop), 2 an activation, 3 a deactivation.
    let accepted = check(&case(0, 0, 1, [3, 1], &[(1, 2)]));
    assert!(accepted.returned && accepted.untouched && accepted.log_count == 0);
    assert_eq!(
        accepted.events(),
        [[1, 1, 0], [2, 1, 1], [2, 1, 2], [2, 1, 2]]
    );
    for edges in [&[(1, 0)][..], &[(0, 2)], &[(2, 3), (3, 0)]] {
        let refused = check(&case(0, 0, 1, [3, 2], edges));
        assert!(!refused.returned && refused.log_count == 0);
        assert_eq!(refused.events(), [[1, 1, 0]]);
    }
    let removed = check(&case(1, 0, 1, [3, 1], &[(1, 0)]));
    assert_eq!(removed.events(), [[3, 1, 1], [3, 1, 2], [3, 1, 2]]);
    let itself = check(&case(0, 0, 0, [3, 1], &[]));
    assert!(!itself.returned && itself.events().is_empty());
    assert_eq!((itself.log_count, itself.log_level), (1, 200));
    assert_eq!(itself.log(), "obs_source_add_active_child: parent == child");
    let removed_itself = check(&case(1, 0, 0, [2, 2], &[]));
    assert_eq!(removed_itself.events(), [[3, 0, 1], [3, 0, 1]]);
    for (which, function) in [
        (0, "obs_source_add_active_child"),
        (1, "obs_source_remove_active_child"),
    ] {
        for (parent, child, name) in [(-1, 1, "parent"), (0, -1, "child"), (-1, -1, "parent")] {
            let null = check(&case(which, parent, child, [3, 1], &[]));
            assert!(!null.returned && null.events().is_empty());
            assert_eq!((null.log_count, null.log_level), (1, 400));
            assert_eq!(null.log(), format!("{function}: Null '{name}' parameter"));
        }
    }
    let mut clobbered = case(0, 0, 1, [3, 2], &[]);
    clobbered.clobber = true;
    assert_eq!(check(&clobbered).events().len(), 4);
    assert_eq!(
        check(&case(0, 0, 1, [2, 2], &[(-1, 1)])).events()[0],
        [1, 1, 0]
    );
}

fn grid_and_mutants() {
    // The grid: every call, NULL, self, cycle shape and counter edge, with
    // and without counters changing under the loop.
    let mut count = 0usize;
    for c in grid() {
        let _ = check(&c);
        count += 1;
    }
    assert_eq!(count, 2 * 2 * 3 * 7 * 6 * EDGE_SETS.len() * 2);

    // Each mutant disagrees with the original somewhere on the grid.
    let mutants: [(&str, Add, Remove); 10] = [
        ("edge parent dropped", add_drop_parent, remove_drop_parent),
        ("edge child dropped", add_drop_child, remove_drop_child),
        ("walk stops at the child", add_stop_root, remove_stop_root),
        ("views swapped", add_swap_views, remove_swap_views),
        (
            "direction swapped",
            add_swap_direction,
            remove_swap_direction,
        ),
        ("silent NULL", add_silent_null, remove_silent_null),
        ("NULL names swapped", add_swap_null, remove_swap_null),
        ("show count reloaded", add_reloading, rust_remove),
        ("no self check", add_no_self_check, rust_remove),
        ("remove walks", rust_add, remove_walking),
    ];
    for (name, add, remove) in mutants {
        assert!(
            grid().any(|c| check_with(add, remove, &c).is_none()),
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
    let edge = (-1i8..4, -1i8..4);
    runner
        .run(
            &(
                (0u8..2, -1i8..4, -1i8..4, any::<u8>()),
                (-4 as c_long..12, -4 as c_long..12),
                proptest::collection::vec(edge, 0..=EDGES),
                any::<bool>(),
            ),
            |((which, parent, child, noise), (show, active), edges, clobber)| {
                let mut c = case(which, parent, child, [show, active], &edges);
                c.noise = noise;
                c.clobber = clobber;
                let out = check(&c);
                prop_assert!(out.untouched);
                prop_assert!(out.events <= u32::try_from(EVENTS).unwrap());
                Ok(())
            },
        )
        .unwrap();
}
