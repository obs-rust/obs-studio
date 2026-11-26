//! Tier 3: the original C `obs_source_get_width()` and
//! `obs_source_get_height()` and the Rust exports, on noise-filled
//! real-header sources, each reaching the same libobs (oracle/
//! source_get_size.c): the production field and lock adapters,
//! `obs_source_size_entry()` and `obs_source_base_width()` / `_height()`,
//! whose size callbacks record which node answered, on which axis, and which
//! nodes' `filter_mutex` was held. Compares those events in order, the
//! result, the log lines and whether every mutex was free afterwards.
//!
//! Excluded input: a NULL entry in a filter list, which
//! `obs_source_filter_add()` never stores; the original C dereferences it
//! inside `obs_source_base_width()`, the Rust answers 0.
use core::ffi::{CStr, c_char, c_void};
use core::ptr::NonNull;
use obs_source::ffi::source_get_size::{
    Libobs, get_with, obs_source_get_height, obs_source_get_width,
};
use obs_source::get_size::{self, Axis, HEIGHT_FUNCTION, WIDTH_FUNCTION};
use obs_source::size_route::{TYPE_FILTER, TYPE_TRANSITION};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

const NODES: usize = 4;
const FILTERS: usize = 3;
const EVENTS: usize = 16;
const INPUT: i32 = 0;
const SCENE: i32 = 3;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct Case {
    which: u8,
    source: i8,
    noise: u8,
    kind: [i32; NODES],
    data: [bool; NODES],
    enabled: [bool; NODES],
    filter_parent: [i8; NODES],
    filter_target: [i8; NODES],
    filters: [u8; NODES],
    filter: [[i8; FILTERS]; NODES],
    size: [[u32; 2]; NODES],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Outcome {
    events: u32,
    trace: [u64; 3 * EVENTS],
    returned: u32,
    log_count: u32,
    log_level: i32,
    log: [c_char; 160],
    unlocked: bool,
}

impl Default for Outcome {
    fn default() -> Self {
        Self {
            events: 0,
            trace: [0; 3 * EVENTS],
            returned: 0,
            log_count: 0,
            log_level: 0,
            log: [0; 160],
            unlocked: false,
        }
    }
}

impl Outcome {
    /// The events as `(node, axis, held mask)` triples.
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

type Get = unsafe extern "C" fn(*mut c_void) -> u32;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

unsafe extern "C" {
    fn oracle_source_get_size_originals(width: *mut Option<Get>, height: *mut Option<Get>);
    fn oracle_source_get_size_run(
        width: Get,
        height: Get,
        get: GetLog,
        set: SetLog,
        case: *const Case,
        out: *mut Outcome,
    ) -> bool;
}

fn originals() -> (Get, Get) {
    let mut width = None;
    let mut height = None;
    // SAFETY: two writable slots for function pointers.
    unsafe { oracle_source_get_size_originals(&raw mut width, &raw mut height) };
    (width.unwrap(), height.unwrap())
}

/// Runs one side; panics when the oracle could not build the fixture or an
/// adapter does not reach its field.
fn run(width: Get, height: Get, case: &Case) -> Outcome {
    let mut out = Outcome::default();
    // SAFETY: the isolated child owns the log handler; the case and the
    // outcome are valid for the call.
    let valid = unsafe {
        oracle_source_get_size_run(
            width,
            height,
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
fn check_with(width: Get, height: Get, case: &Case) -> Option<Outcome> {
    let (original_width, original_height) = originals();
    let original = run(original_width, original_height, case);
    let ported = run(width, height, case);
    (ported == original).then_some(original)
}

fn check(case: &Case) -> Outcome {
    check_with(obs_source_get_width, obs_source_get_height, case)
        .unwrap_or_else(|| panic!("the Rust exports disagree with the original C: {case:?}"))
}

/// Node 0 of `kind` with `filters` (node indices), nodes 1 and 2 enabled
/// filters of node 0 (node 1 targets node 3), node 3 an input; each node
/// answers `100 + i` wide and `200 + i` high.
fn case(which: u8, source: i8, kind: i32, filters: &[i8]) -> Case {
    let mut case = Case {
        which,
        source,
        noise: 0xa5,
        kind: [kind, TYPE_FILTER, TYPE_FILTER, INPUT],
        data: [true; NODES],
        enabled: [true; NODES],
        filter_parent: [-1, 0, 0, -1],
        filter_target: [-1, 3, -1, -1],
        filters: [u8::try_from(filters.len()).unwrap(), 0, 0, 0],
        ..Case::default()
    };
    for (i, f) in filters.iter().enumerate() {
        case.filter[0][i] = *f;
    }
    for (i, size) in case.size.iter_mut().enumerate() {
        let i = u32::try_from(i).unwrap();
        *size = [100 + i, 200 + i];
    }
    case
}

#[derive(Clone, Copy)]
enum Change {
    NoLock,
    FilterAsInput,
    IgnoreFilters,
    SilentNull,
    SwapNullName,
    SwapAxis,
}

struct Mutant(Change);

impl get_size::Libobs for Mutant {
    type Source = NonNull<c_void>;

    fn log_null(&mut self, function: &'static CStr) {
        match self.0 {
            Change::SilentNull => {}
            Change::SwapNullName => Libobs.log_null(if function == WIDTH_FUNCTION {
                HEIGHT_FUNCTION
            } else {
                WIDTH_FUNCTION
            }),
            _ => Libobs.log_null(function),
        }
    }
    fn has_data(&mut self, source: Self::Source) -> bool {
        Libobs.has_data(source)
    }
    fn source_type(&mut self, source: Self::Source) -> i32 {
        match (self.0, Libobs.source_type(source)) {
            (Change::FilterAsInput, TYPE_FILTER) => INPUT,
            (_, kind) => kind,
        }
    }
    fn lock_filters(&mut self, source: Self::Source) {
        if !matches!(self.0, Change::NoLock) {
            Libobs.lock_filters(source);
        }
    }
    fn unlock_filters(&mut self, source: Self::Source) {
        if !matches!(self.0, Change::NoLock) {
            Libobs.unlock_filters(source);
        }
    }
    fn has_filters(&mut self, source: Self::Source) -> bool {
        !matches!(self.0, Change::IgnoreFilters) && Libobs.has_filters(source)
    }
    fn first_filter(&mut self, source: Self::Source) -> Option<Self::Source> {
        Libobs.first_filter(source)
    }
    fn base_size(&mut self, source: Self::Source, axis: Axis) -> u32 {
        let axis = match (self.0, axis) {
            (Change::SwapAxis, Axis::Width) => Axis::Height,
            (Change::SwapAxis, Axis::Height) => Axis::Width,
            (_, axis) => axis,
        };
        Libobs.base_size(source, axis)
    }
}

macro_rules! mutant {
    ($width:ident, $height:ident, $change:expr) => {
        unsafe extern "C" fn $width(source: *mut c_void) -> u32 {
            get_with(&mut Mutant($change), source, Axis::Width)
        }
        unsafe extern "C" fn $height(source: *mut c_void) -> u32 {
            get_with(&mut Mutant($change), source, Axis::Height)
        }
    };
}

mutant!(width_no_lock, height_no_lock, Change::NoLock);
mutant!(
    width_filter_input,
    height_filter_input,
    Change::FilterAsInput
);
mutant!(width_ignore, height_ignore, Change::IgnoreFilters);
mutant!(width_silent, height_silent, Change::SilentNull);
mutant!(width_swap_name, height_swap_name, Change::SwapNullName);
mutant!(width_swap_axis, height_swap_axis, Change::SwapAxis);

const FILTER_LISTS: [&[i8]; 5] = [&[], &[1], &[2], &[2, 1], &[1, 2, 3]];

fn grid() -> impl Iterator<Item = Case> {
    let mut cases = Vec::new();
    for which in 0..2u8 {
        for source in [-1i8, 0, 1, 3] {
            for kind in [INPUT, TYPE_FILTER, TYPE_TRANSITION, SCENE, 7] {
                for filters in FILTER_LISTS {
                    for data in [true, false] {
                        for enabled in [true, false] {
                            for parent in [-1i8, 3] {
                                let mut c = case(which, source, kind, filters);
                                c.data[0] = data;
                                c.enabled[0] = enabled;
                                c.enabled[1] = enabled;
                                c.filter_parent[0] = parent;
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
fn get_size_matches_c() {
    if !crate::output::isolated(concat!(module_path!(), "::get_size_matches_c")) {
        return;
    }
    pinned();
    grid_and_mutants();
    random_cases();
}

fn pinned() {
    // Pinned answers: the shared result itself, so a change on both sides
    // (or in the oracle) still fails. Events are (node, axis, held mask).
    let plain = check(&case(0, 0, INPUT, &[]));
    assert_eq!((plain.returned, plain.log_count), (100, 0));
    assert!(plain.unlocked);
    assert_eq!(plain.events(), [[0, 0, 0b0001]]);
    let filtered = check(&case(1, 0, SCENE, &[2, 1]));
    assert_eq!(filtered.returned, 202);
    assert_eq!(filtered.events(), [[2, 1, 0b0001]]);
    let filter = check(&case(0, 1, INPUT, &[1]));
    assert_eq!(filter.returned, 101);
    assert_eq!(filter.events(), [[1, 0, 0]]);
    let mut no_data = case(0, 0, INPUT, &[1]);
    no_data.data[0] = false;
    let no_data = check(&no_data);
    assert_eq!((no_data.returned, no_data.log_count), (0, 0));
    assert!(no_data.events().is_empty());
    for (which, function) in [(0, "obs_source_get_width"), (1, "obs_source_get_height")] {
        let null = check(&case(which, -1, INPUT, &[]));
        assert_eq!(null.returned, 0);
        assert!(null.events().is_empty());
        assert_eq!((null.log_count, null.log_level), (1, 400));
        assert_eq!(null.log(), format!("{function}: Null 'source' parameter"));
    }
}

fn grid_and_mutants() {
    let mut count = 0usize;
    for c in grid() {
        let out = check(&c);
        assert!(out.unlocked);
        count += 1;
    }
    assert_eq!(count, 2 * 4 * 5 * FILTER_LISTS.len() * 2 * 2 * 2);

    // Each mutant disagrees with the original somewhere on the grid.
    let mutants: [(&str, Get, Get); 6] = [
        ("no lock", width_no_lock, height_no_lock),
        ("filter as input", width_filter_input, height_filter_input),
        ("filters ignored", width_ignore, height_ignore),
        ("silent NULL", width_silent, height_silent),
        ("NULL names swapped", width_swap_name, height_swap_name),
        ("axes swapped", width_swap_axis, height_swap_axis),
    ];
    for (name, width, height) in mutants {
        assert!(
            grid().any(|c| check_with(width, height, &c).is_none()),
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
                (0u8..2, -1i8..4, any::<u8>()),
                proptest::array::uniform4(-1i32..5),
                proptest::array::uniform4(any::<bool>()),
                proptest::array::uniform4(any::<bool>()),
                proptest::collection::vec(0i8..4, 0..=FILTERS),
                proptest::array::uniform4(any::<[u32; 2]>()),
            ),
            |((which, source, noise), kind, data, enabled, filters, size)| {
                let mut c = case(which, source, INPUT, &filters);
                c.noise = noise;
                c.data = data;
                c.enabled = enabled;
                c.size = size;
                // Only node 0 lists filters; its type varies, the others
                // keep the filter chain 1 -> 3 well-founded.
                c.kind[0] = kind[0];
                c.kind[3] = if kind[3] == TYPE_FILTER {
                    INPUT
                } else {
                    kind[3]
                };
                let out = check(&c);
                prop_assert!(out.unlocked);
                prop_assert!(out.events <= u32::try_from(EVENTS).unwrap());
                Ok(())
            },
        )
        .unwrap();
}
