//! Tier 3: the original C `obs_source_filter_remove()` and
//! `obs_source_filter_remove_refless()` against the Rust exports on
//! noise-filled real-header sources, with a real signal handler on the
//! source and on a stand-in core: the list, every filter's target and
//! parent, the return value, every signal (calldata, the list's length, the
//! lock state), the callback (the parent it got, the filter's parent then),
//! the release, the log lines in order with the rest, every other byte, the
//! lock afterwards and the last log line.
use core::ffi::{CStr, c_char, c_long, c_uint, c_void};
use core::ptr::NonNull;
use obs_source::ffi::source_filter_remove::{
    obs_source_filter_remove, obs_source_filter_remove_refless,
};
use obs_source::filter_remove::{FilterList, remove, remove_refless};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Remove = unsafe extern "C" fn(*mut c_void, *mut c_void);
type Refless = unsafe extern "C" fn(*mut c_void, *mut c_void) -> bool;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

unsafe extern "C" {
    fn oracle_sfr_run(
        remove: Option<Remove>,
        refless_fn: Option<Refless>,
        get: GetLog,
        set: SetLog,
        refless: bool,
        present: bool,
        noise: u8,
        count: usize,
        callbacks: c_uint,
        which: usize,
        out: *mut c_long,
        message: *mut c_char,
        message_size: usize,
    ) -> bool;
    fn oracle_sfr_layout(which: usize) -> usize;
    fn obs_source_filter_remove_lock(source: *mut c_void);
    fn obs_source_filter_remove_count(source: *const c_void) -> usize;
    fn obs_source_filter_remove_at(source: *const c_void, index: usize) -> *mut c_void;
    fn obs_source_filter_remove_target(filter: *const c_void) -> *mut c_void;
    fn obs_source_filter_remove_set_target(filter: *mut c_void, target: *mut c_void);
    fn obs_source_filter_remove_erase(source: *mut c_void, index: usize);
    fn obs_source_filter_remove_unlock(source: *mut c_void);
    fn obs_source_filter_remove_signal(source: *mut c_void, filter: *mut c_void);
    fn obs_source_filter_remove_log(source: *mut c_void, filter: *mut c_void);
    fn obs_source_filter_remove_has_callback(filter: *const c_void) -> bool;
    fn obs_source_filter_remove_callback(filter: *mut c_void);
    fn obs_source_filter_remove_clear_links(filter: *mut c_void);
    fn obs_source_filter_remove_release(filter: *mut c_void);
}

/// The most filters a chain gets (`SFR_FILTERS`).
const FILTERS: usize = 7;
/// `which` for the filter on no list.
const OUTSIDER: usize = FILTERS;
/// `which` for a NULL filter.
const NULL_FILTER: usize = FILTERS + 1;
/// The event width (`SFR_EVENT_WIDTH`) and the out array's layout.
const EVENT_WIDTH: usize = 5;
const EVENTS: usize = 16;
const RETURNED: usize = 0;
const COUNT_AFTER: usize = 1;
const UNCHANGED: usize = 2 + FILTERS + 2 * (FILTERS + 1);
const UNLOCKED: usize = UNCHANGED + 1;
const LOGS: usize = UNCHANGED + 2;
const EVENT_COUNT: usize = UNCHANGED + 3;
const OVERFLOW: usize = UNCHANGED + 4;
const HEAD: usize = UNCHANGED + 5;
const OUT: usize = HEAD + EVENTS * EVENT_WIDTH;
/// Event kinds (`SFR_EV_*`).
const EV_GLOBAL_SIGNAL: c_long = 1;
const EV_SOURCE_SIGNAL: c_long = 2;
const EV_CALLBACK: c_long = 3;
const EV_RELEASE: c_long = 4;
const EV_LOG: c_long = 5;

/// What one call left: the out array and the last log line.
type Seen = (Vec<c_long>, String);

#[derive(Clone, Copy, Debug)]
struct Case {
    refless: bool,
    present: bool,
    noise: u8,
    count: usize,
    callbacks: c_uint,
    which: usize,
}

fn run(remove: Option<Remove>, refless: Option<Refless>, case: Case) -> Seen {
    let mut out: [c_long; OUT] = [0; OUT];
    let mut buffer = [0 as c_char; 128];
    // SAFETY: the isolated child owns the log handler. C builds a live
    // fixture, runs the call, records and frees it, and restores logging;
    // both buffers are as long as passed.
    let built = unsafe {
        oracle_sfr_run(
            remove,
            refless,
            base_get_log_handler,
            base_set_log_handler,
            case.refless,
            case.present,
            case.noise,
            case.count,
            case.callbacks,
            case.which,
            out.as_mut_ptr(),
            buffer.as_mut_ptr(),
            buffer.len(),
        )
    };
    assert!(built, "{case:?}: fixture");
    // SAFETY: the oracle NUL-terminated the buffer.
    let message = unsafe { CStr::from_ptr(buffer.as_ptr()) }
        .to_str()
        .unwrap()
        .to_owned();
    (out.to_vec(), message)
}

/// Both sides alike on `case`; the original's observation.
fn compare_with(remove: Remove, refless: Refless, case: Case) -> Option<Seen> {
    let c = run(None, None, case);
    let rust = run(Some(remove), Some(refless), case);
    (c == rust).then_some(c)
}

/// The kinds of the recorded events, in order.
fn kinds(out: &[c_long]) -> Vec<c_long> {
    let count = usize::try_from(out[EVENT_COUNT]).unwrap();
    (0..count).map(|i| out[HEAD + i * EVENT_WIDTH]).collect()
}

fn compare(case: Case) -> Seen {
    let seen = compare_with(
        obs_source_filter_remove,
        obs_source_filter_remove_refless,
        case,
    )
    .unwrap_or_else(|| {
        panic!(
            "{case:?}: C {:?}, Rust {:?}",
            run(None, None, case),
            run(
                Some(obs_source_filter_remove),
                Some(obs_source_filter_remove_refless),
                case
            )
        )
    });
    let out = &seen.0;
    // Nothing else written, the lock free, no event lost, and no event under
    // the lock.
    assert_eq!(
        (out[UNCHANGED], out[UNLOCKED], out[OVERFLOW]),
        (1, 1, 0),
        "{case:?}"
    );
    let count = usize::try_from(out[EVENT_COUNT]).unwrap();
    for i in 0..count {
        assert_eq!(
            out[HEAD + i * EVENT_WIDTH + 4],
            0,
            "{case:?}: event {i} locked"
        );
    }
    seen
}

/// Mutants of the exports: the real core over a list that breaks one call.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Break {
    NoRetarget,
    NoClear,
    SignalLocked,
    NoCallback,
    NoRelease,
}

struct Mutant {
    source: NonNull<c_void>,
    broken: Break,
}

impl FilterList for Mutant {
    type Source = *mut c_void;
    fn lock(&self) {
        // SAFETY: a live fixture source.
        unsafe { obs_source_filter_remove_lock(self.source.as_ptr()) }
    }
    fn count(&self) -> usize {
        // SAFETY: as the export's.
        unsafe { obs_source_filter_remove_count(self.source.as_ptr()) }
    }
    fn filter(&self, index: usize) -> *mut c_void {
        // SAFETY: as the export's.
        unsafe { obs_source_filter_remove_at(self.source.as_ptr(), index) }
    }
    fn target(&self, filter: *mut c_void) -> *mut c_void {
        // SAFETY: as the export's.
        unsafe { obs_source_filter_remove_target(filter) }
    }
    fn set_target(&self, filter: *mut c_void, target: *mut c_void) {
        if self.broken != Break::NoRetarget {
            // SAFETY: as the export's.
            unsafe { obs_source_filter_remove_set_target(filter, target) }
        }
    }
    fn erase(&self, index: usize) {
        // SAFETY: as the export's.
        unsafe { obs_source_filter_remove_erase(self.source.as_ptr(), index) }
    }
    fn unlock(&self) {
        // SAFETY: as the export's.
        unsafe { obs_source_filter_remove_unlock(self.source.as_ptr()) }
    }
    fn signal(&self, filter: *mut c_void) {
        if self.broken == Break::SignalLocked {
            // SAFETY: as the export's, but under the lock.
            unsafe {
                obs_source_filter_remove_lock(self.source.as_ptr());
                obs_source_filter_remove_signal(self.source.as_ptr(), filter);
                obs_source_filter_remove_unlock(self.source.as_ptr());
            }
        } else {
            // SAFETY: as the export's.
            unsafe { obs_source_filter_remove_signal(self.source.as_ptr(), filter) }
        }
    }
    fn log_removed(&self, filter: *mut c_void) {
        // SAFETY: as the export's.
        unsafe { obs_source_filter_remove_log(self.source.as_ptr(), filter) }
    }
    fn has_remove_callback(&self, filter: *mut c_void) -> bool {
        // SAFETY: as the export's.
        self.broken != Break::NoCallback && unsafe { obs_source_filter_remove_has_callback(filter) }
    }
    fn call_remove_callback(&self, filter: *mut c_void) {
        // SAFETY: as the export's.
        unsafe { obs_source_filter_remove_callback(filter) }
    }
    fn clear_links(&self, filter: *mut c_void) {
        if self.broken != Break::NoClear {
            // SAFETY: as the export's.
            unsafe { obs_source_filter_remove_clear_links(filter) }
        }
    }
    fn release(&self, filter: *mut c_void) {
        if self.broken != Break::NoRelease {
            // SAFETY: as the export's.
            unsafe { obs_source_filter_remove_release(filter) }
        }
    }
}

fn broken(b: u8) -> Break {
    match b {
        0 => Break::NoRetarget,
        1 => Break::NoClear,
        2 => Break::SignalLocked,
        3 => Break::NoCallback,
        _ => Break::NoRelease,
    }
}

unsafe extern "C" fn mutant_remove<const B: u8>(source: *mut c_void, filter: *mut c_void) {
    let (Some(source), false) = (NonNull::new(source), filter.is_null()) else {
        // SAFETY: as the export's.
        return unsafe { obs_source_filter_remove(source, filter) };
    };
    let mutant = Mutant {
        source,
        broken: broken(B),
    };
    remove(Some(&mutant), Some(filter), |_, _| panic!("logged"));
}

unsafe extern "C" fn mutant_refless<const B: u8>(source: *mut c_void, filter: *mut c_void) -> bool {
    let Some(source) = NonNull::new(source) else {
        return false;
    };
    let mutant = Mutant {
        source,
        broken: broken(B),
    };
    remove_refless(&mutant, filter)
}

/// Each mutant disagrees with the original C somewhere.
fn rejects_mutants() {
    let mutants: [(Remove, Refless); 5] = [
        (mutant_remove::<0>, mutant_refless::<0>),
        (mutant_remove::<1>, mutant_refless::<1>),
        (mutant_remove::<2>, mutant_refless::<2>),
        (mutant_remove::<3>, mutant_refless::<3>),
        (mutant_remove::<4>, mutant_refless::<4>),
    ];
    for (remove, refless) in mutants {
        let caught = (0..4).any(|which| {
            compare_with(
                remove,
                refless,
                Case {
                    refless: false,
                    present: true,
                    noise: 0x5a,
                    count: 4,
                    callbacks: 0b1111,
                    which,
                },
            )
            .is_none()
        });
        assert!(caught, "a filter_remove mutant agreed with the original C");
    }
}

const SOURCE_LINE: &str = "obs_source_filter_remove: Null 'source' parameter";
const FILTER_LINE: &str = "obs_source_filter_remove: Null 'filter' parameter";

/// NULL source or filter: one line, nothing else.
fn null_arguments() {
    for (present, which, line) in [
        (false, 0, SOURCE_LINE),
        (false, NULL_FILTER, SOURCE_LINE),
        (true, NULL_FILTER, FILTER_LINE),
    ] {
        let case = Case {
            refless: false,
            present,
            noise: 0x5a,
            count: 3,
            callbacks: 0b111,
            which,
        };
        let (out, message) = compare(case);
        assert_eq!(message, line);
        assert_eq!(kinds(&out), [EV_LOG]);
        assert_eq!(out[LOGS], 1);
        assert_eq!(out[COUNT_AFTER], 3);
    }
}

/// One grid case, compared and checked against the contract; whether the
/// filter was removed.
fn grid_case(case: Case) -> bool {
    let (out, message) = compare(case);
    let events = kinds(&out);
    let listed = case.which < case.count;
    assert_eq!(out[LOGS], c_long::from(listed), "{case:?}");
    if !listed {
        assert!(events.is_empty(), "{case:?}");
        assert_eq!(out[RETURNED], 0);
        assert_eq!(out[COUNT_AFTER], c_long::try_from(case.count).unwrap());
        return false;
    }
    let names = ["a", "bb", "", "dddd", "e", "ffffff", "g"];
    assert_eq!(
        message,
        format!(
            "- filter '{}' (f{}) removed from source 'parent'",
            names[case.which], case.which
        )
    );
    assert_eq!(out[COUNT_AFTER], c_long::try_from(case.count - 1).unwrap());
    assert_eq!(out[RETURNED], c_long::from(case.refless));
    let mut expected = vec![EV_GLOBAL_SIGNAL, EV_SOURCE_SIGNAL, EV_LOG];
    if case.callbacks & (1 << case.which) != 0 {
        expected.push(EV_CALLBACK);
    }
    if !case.refless {
        expected.push(EV_RELEASE);
    }
    assert_eq!(events, expected, "{case:?}");
    true
}

/// Every chain of up to 7 filters, every listed filter, the one on no list
/// and NULL, through both entry points, with every callback pattern
/// (sampled past 4 filters).
fn every_chain() {
    let mut grid = 0u64;
    let mut removed = 0usize;
    for count in 0..=FILTERS {
        for callbacks in 0..1u32 << (count + 1) {
            for which in (0..count).chain([OUTSIDER, NULL_FILTER]) {
                for refless in [false, true] {
                    grid += 1;
                    let sampled_out = count >= 5 && !obs_c_oracle::parity::keep(grid);
                    // A NULL filter through obs_source_filter_remove() is
                    // null_arguments()'s.
                    if sampled_out || (which == NULL_FILTER && !refless) {
                        continue;
                    }
                    let noise = u8::try_from(grid % 256).unwrap().wrapping_mul(37);
                    let case = Case {
                        refless,
                        present: true,
                        noise,
                        count,
                        callbacks,
                        which,
                    };
                    removed += usize::from(grid_case(case));
                }
            }
        }
    }
    assert!(removed > 0);
}

#[test]
fn filter_remove_matches_c() {
    if !crate::output::isolated(concat!(module_path!(), "::filter_remove_matches_c")) {
        return;
    }
    // SAFETY: plain reads of the oracle's constants.
    assert_eq!(
        unsafe {
            [
                oracle_sfr_layout(0),
                oracle_sfr_layout(1),
                oracle_sfr_layout(2),
            ]
        },
        [OUT, FILTERS, EVENT_WIDTH]
    );
    null_arguments();
    every_chain();
    rejects_mutants();
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(
            &(
                any::<(bool, bool, u8, c_uint)>(),
                0..=FILTERS,
                0..=NULL_FILTER,
            ),
            |((refless, present, noise, callbacks), count, which)| {
                compare(Case {
                    refless,
                    // obs_source_destroy() never passes a NULL parent.
                    present: present || refless,
                    noise,
                    count,
                    callbacks: callbacks & 0xff,
                    which,
                });
                Ok(())
            },
        )
        .unwrap();
}
