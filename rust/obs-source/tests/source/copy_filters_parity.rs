//! Tier 3: the original C `obs_source_copy_filters()`,
//! `obs_source_copy_single_filter()` and `duplicate_filters()` against the
//! Rust exports, on noise-filled real-header sources, with the libobs calls
//! the copies make recorded by the oracle's stand-ins: every call in order
//! with its arguments and the lock state at it, every fixture byte, the
//! lock afterwards and the exact log line.
use core::ffi::{CStr, c_char, c_int, c_long, c_uint, c_void};
use obs_source::ffi::source_copy_filters::{
    obs_source_copy_filters, obs_source_copy_single_filter, obs_source_duplicate_filters,
};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Pair = unsafe extern "C" fn(*mut c_void, *mut c_void);
type Triple = unsafe extern "C" fn(*mut c_void, *mut c_void, bool);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

unsafe extern "C" {
    fn oracle_scf_run(
        pair: Option<Pair>,
        triple: Option<Triple>,
        get: GetLog,
        set: SetLog,
        which: c_int,
        noise: u8,
        count: usize,
        private: bool,
        private_arg: bool,
        dst_arg: c_int,
        second_arg: c_int,
        dying: c_uint,
        disabled: c_uint,
        incompatible: c_uint,
        no_duplicate: c_uint,
        out: *mut c_long,
        message: *mut c_char,
        message_size: usize,
    ) -> bool;
    fn oracle_scf_layout(which: usize) -> usize;
    fn obs_source_copy_filters_lock(src: *mut c_void);
    fn obs_source_copy_filters_count(src: *const c_void) -> usize;
    fn obs_source_copy_filters_at(src: *const c_void, index: usize) -> *mut c_void;
    fn obs_source_copy_filters_get_ref(filter: *mut c_void) -> *mut c_void;
    fn obs_source_copy_filters_unlock(src: *mut c_void);
    fn obs_source_copy_filters_new_name(dst: *mut c_void, filter: *mut c_void) -> *mut c_char;
    fn obs_source_copy_filters_enabled(filter: *const c_void) -> bool;
    fn obs_source_copy_filters_duplicate(
        filter: *mut c_void,
        name: *const c_char,
        private: bool,
    ) -> *mut c_void;
    fn obs_source_copy_filters_set_enabled(copy: *mut c_void, enabled: bool);
    fn obs_source_copy_filters_free_name(name: *mut c_char);
    fn obs_source_copy_filters_add(dst: *mut c_void, copy: *mut c_void);
    fn obs_source_copy_filters_release(source: *mut c_void);
}

/// The most filters a source gets (`SCF_FILTERS`).
const FILTERS: usize = 7;
/// The event width (`SCF_EVENT_WIDTH`) and the out array's length.
const WIDTH: usize = 5;
const HEAD: usize = 6;
const OUT: usize = HEAD + 160 * WIDTH;
/// Indexes of the head: events, overflow, logs, log level, unchanged,
/// unlocked.
const EVENTS: usize = 0;
const OVERFLOW: usize = 1;
const LOGS: usize = 2;
const LOG_LEVEL: usize = 3;
const UNCHANGED: usize = 4;
const UNLOCKED: usize = 5;

/// `which`.
const COPY_FILTERS: c_int = 0;
const COPY_SINGLE: c_int = 1;
const DUPLICATE_FILTERS: c_int = 2;
/// Arguments: NULL, the destination, the source, then filter `i` at 3 + i.
const NULL: c_int = 0;
const DST: c_int = 1;
const SRC: c_int = 2;
const FILTER: c_int = 3;

/// Event kinds (`SCF_EV_*`).
const GET_REF: c_long = 1;
const ADD: c_long = 8;
const RELEASE: c_long = 9;

/// Ids in the trace: the destination, then filters from 2, copies from 9.
const ID_DST: c_long = 0;
const ID_FILTER0: c_long = 2;
const ID_COPY0: c_long = ID_FILTER0 + 7;

#[derive(Clone, Copy, Debug)]
struct Case {
    which: c_int,
    noise: u8,
    count: usize,
    private: bool,
    private_arg: bool,
    dst_arg: c_int,
    second_arg: c_int,
    dying: c_uint,
    disabled: c_uint,
    incompatible: c_uint,
    no_duplicate: c_uint,
}

/// Serializes the cases of this file: the log handler is process-wide.
static LOG: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The out array and the log line of one call; `None` runs the original C.
fn run(case: Case, pair: Option<Pair>, triple: Option<Triple>) -> (Vec<c_long>, String) {
    let _log = LOG
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut out = vec![0; OUT];
    let mut message = [0 as c_char; 128];
    // SAFETY: the oracle builds and frees its own fixture, writes OUT values
    // and a NUL-terminated message within the sizes given, and runs the
    // functions only on that fixture.
    let built = unsafe {
        oracle_scf_run(
            pair,
            triple,
            base_get_log_handler,
            base_set_log_handler,
            case.which,
            case.noise,
            case.count,
            case.private,
            case.private_arg,
            case.dst_arg,
            case.second_arg,
            case.dying,
            case.disabled,
            case.incompatible,
            case.no_duplicate,
            out.as_mut_ptr(),
            message.as_mut_ptr(),
            message.len(),
        )
    };
    assert!(built, "{case:?}");
    // SAFETY: the oracle NUL-terminates the message.
    let message = unsafe { CStr::from_ptr(message.as_ptr()) };
    (out, message.to_string_lossy().into_owned())
}

fn rust_pair(which: c_int) -> Pair {
    if which == COPY_FILTERS {
        obs_source_copy_filters
    } else {
        obs_source_copy_single_filter
    }
}

/// The original C and the Rust export on the same case.
fn both(case: Case) -> ((Vec<c_long>, String), (Vec<c_long>, String)) {
    let c = run(case, None, None);
    let rust = if case.which == DUPLICATE_FILTERS {
        run(case, None, Some(obs_source_duplicate_filters))
    } else {
        run(case, Some(rust_pair(case.which)), None)
    };
    (c, rust)
}

fn check(case: Case) -> (Vec<c_long>, String) {
    let (c, rust) = both(case);
    assert_eq!(c, rust, "{case:?}");
    let (out, message) = c;
    assert_eq!(out[OVERFLOW], 0, "{case:?}");
    assert_eq!(out[UNCHANGED], 1, "{case:?}");
    assert_eq!(out[UNLOCKED], 1, "{case:?}");
    (out, message)
}

fn events(out: &[c_long]) -> Vec<[c_long; WIDTH]> {
    let count = usize::try_from(out[EVENTS]).unwrap();
    (0..count)
        .map(|i| {
            let mut e = [0; WIDTH];
            e.copy_from_slice(&out[HEAD + i * WIDTH..HEAD + (i + 1) * WIDTH]);
            e
        })
        .collect()
}

fn base(which: c_int, count: usize) -> Case {
    Case {
        which,
        noise: 0xa5,
        count,
        private: false,
        private_arg: false,
        dst_arg: DST,
        second_arg: if which == COPY_SINGLE { FILTER } else { SRC },
        dying: 0,
        disabled: 0,
        incompatible: 0,
        no_duplicate: 0,
    }
}

#[test]
fn layout_matches_the_oracle() {
    // SAFETY: plain size queries.
    unsafe {
        assert_eq!(oracle_scf_layout(0), OUT);
        assert_eq!(oracle_scf_layout(1), FILTERS);
        assert_eq!(oracle_scf_layout(2), WIDTH);
    }
}

/// The NULL lines: one LOG_DEBUG line, the destination checked first, and
/// no libobs call.
#[test]
fn null_arguments_match_c() {
    let lines = [
        (
            COPY_FILTERS,
            NULL,
            SRC,
            "obs_source_copy_filters: Null 'dst' parameter",
        ),
        (
            COPY_FILTERS,
            DST,
            NULL,
            "obs_source_copy_filters: Null 'src' parameter",
        ),
        (
            COPY_FILTERS,
            NULL,
            NULL,
            "obs_source_copy_filters: Null 'dst' parameter",
        ),
        (
            COPY_SINGLE,
            NULL,
            FILTER,
            "obs_source_copy_single_filter: Null 'dst' parameter",
        ),
        (
            COPY_SINGLE,
            DST,
            NULL,
            "obs_source_copy_single_filter: Null 'filter' parameter",
        ),
        (
            COPY_SINGLE,
            NULL,
            NULL,
            "obs_source_copy_single_filter: Null 'dst' parameter",
        ),
    ];
    for (which, dst_arg, second_arg, line) in lines {
        let case = Case {
            dst_arg,
            second_arg,
            ..base(which, 3)
        };
        let (out, message) = check(case);
        assert_eq!(message, line);
        assert_eq!(out[LOGS], 1);
        assert_eq!(out[LOG_LEVEL], 400);
        assert_eq!(out[EVENTS], 0);
    }
}

/// Every count, private flag, argument pair and mask pattern: the same
/// calls in the same order, with the same lock state, as the original.
#[test]
fn every_case_matches_c() {
    let masks: [c_uint; 4] = [0, 0b101, 0b10, 0x7f];
    let mut runs = 0;
    for which in [COPY_FILTERS, COPY_SINGLE, DUPLICATE_FILTERS] {
        let pairs: &[(c_int, c_int)] = match which {
            COPY_FILTERS => &[(DST, SRC), (SRC, SRC)],
            COPY_SINGLE => &[
                (DST, FILTER),
                (DST, FILTER + 2),
                (DST, FILTER + 6),
                (SRC, FILTER),
            ],
            _ => &[(DST, SRC), (SRC, SRC), (NULL, SRC)],
        };
        for count in 0..=FILTERS {
            for private in [false, true] {
                for &(dst_arg, second_arg) in pairs {
                    for dying in masks {
                        for disabled in [0, 0b110] {
                            for incompatible in [0, 0b11] {
                                for no_duplicate in [0, 0b1001] {
                                    let case = Case {
                                        private,
                                        private_arg: !private,
                                        dst_arg,
                                        second_arg,
                                        dying,
                                        disabled,
                                        incompatible,
                                        no_duplicate,
                                        ..base(which, count)
                                    };
                                    let (out, _) = check(case);
                                    assert_eq!(out[LOGS], 0, "{case:?}");
                                    runs += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(runs, 4608);
}

/// What the original does, pinned: three filters, the middle one going
/// away, copied from the last; every add after the unlock.
#[test]
fn copy_filters_trace_is_pinned() {
    let case = Case {
        dying: 0b010,
        private: true,
        ..base(COPY_FILTERS, 3)
    };
    let (out, _) = check(case);
    let events = events(&out);
    let refs: Vec<_> = events.iter().filter(|e| e[0] == GET_REF).collect();
    assert_eq!(refs.len(), 3);
    assert!(refs.iter().all(|e| e[4] == 1), "references under the lock");
    let adds: Vec<_> = events
        .iter()
        .filter(|e| e[0] == ADD)
        .map(|e| (e[1], e[2], e[4]))
        .collect();
    assert_eq!(adds, [(ID_DST, ID_COPY0 + 2, 0), (ID_DST, ID_COPY0, 0)]);
    let releases: Vec<_> = events
        .iter()
        .filter(|e| e[0] == RELEASE)
        .map(|e| e[1])
        .collect();
    assert_eq!(
        releases,
        [ID_COPY0 + 2, ID_FILTER0 + 2, ID_COPY0, ID_FILTER0]
    );
}

/// A mutant copying first to last.
unsafe extern "C" fn forward_mutant(dst: *mut c_void, src: *mut c_void) {
    // SAFETY: the oracle's live fixture, through its adapters.
    unsafe {
        obs_source_copy_filters_lock(src);
        let mut filters = Vec::new();
        let mut i = 0;
        while i < obs_source_copy_filters_count(src) {
            let filter = obs_source_copy_filters_get_ref(obs_source_copy_filters_at(src, i));
            if !filter.is_null() {
                filters.push(filter);
            }
            i += 1;
        }
        obs_source_copy_filters_unlock(src);
        for filter in filters {
            copy_mutant_one(dst, filter, false);
            obs_source_copy_filters_release(filter);
        }
    }
}

/// A mutant that copies the single filter without checking it.
unsafe extern "C" fn unchecked_single_mutant(dst: *mut c_void, filter: *mut c_void) {
    if !dst.is_null() && !filter.is_null() {
        // SAFETY: the oracle's live fixture, through its adapters.
        unsafe { copy_mutant_one(dst, filter, true) }
    }
}

unsafe fn copy_mutant_one(dst: *mut c_void, filter: *mut c_void, private: bool) {
    // SAFETY: the oracle's live fixture, through its adapters.
    unsafe {
        let name = obs_source_copy_filters_new_name(dst, filter);
        let enabled = obs_source_copy_filters_enabled(filter);
        let copy = obs_source_copy_filters_duplicate(filter, name, private);
        obs_source_copy_filters_set_enabled(copy, enabled);
        obs_source_copy_filters_free_name(name);
        obs_source_copy_filters_add(dst, copy);
        obs_source_copy_filters_release(copy);
    }
}

#[test]
fn the_comparison_rejects_mutants() {
    let case = base(COPY_FILTERS, 3);
    assert_ne!(run(case, None, None), run(case, Some(forward_mutant), None));
    let case = Case {
        incompatible: 1,
        ..base(COPY_SINGLE, 1)
    };
    assert_ne!(
        run(case, None, None),
        run(case, Some(unchecked_single_mutant), None)
    );
}

proptest! {
    #[test]
    fn random_cases_match_c(
        which in 0..3,
        noise in any::<u8>(),
        count in 0..=FILTERS,
        private in any::<bool>(),
        private_arg in any::<bool>(),
        dst_arg in 1..3,
        filter in 0..7,
        dying in 0u32..128,
        disabled in 0u32..128,
        incompatible in 0u32..128,
        no_duplicate in 0u32..128,
    ) {
        let second_arg = if which == COPY_SINGLE { FILTER + filter } else { SRC };
        let case = Case {
            which,
            noise,
            count,
            private,
            private_arg,
            dst_arg,
            second_arg,
            dying,
            disabled,
            incompatible,
            no_duplicate,
        };
        let (c, rust) = both(case);
        prop_assert_eq!(c, rust);
    }
}
