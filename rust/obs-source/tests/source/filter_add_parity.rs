//! Tier 3: the original C `obs_source_filter_add()` against the Rust export
//! on noise-filled real-header sources, with a real signal handler on the
//! source and on a stand-in core and a recording `obs_source_get_ref()`: the
//! list, every filter's target and parent, the reference, every signal
//! (calldata, the list then, the lock state), the callback (the parent it
//! got, the filter's links then), the log lines in order with the rest,
//! every other byte, the lock afterwards and the last log line.
use core::ffi::{CStr, c_char, c_long, c_uint, c_void};
use core::ptr::NonNull;
use obs_source::ffi::source_filter_add::obs_source_filter_add;
use obs_source::filter_add::{FilterList, add};
use obs_source::source_filter_chain::{ASYNC, AUDIO, VIDEO, compatible};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Add = unsafe extern "C" fn(*mut c_void, *mut c_void);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

unsafe extern "C" {
    fn oracle_sfa_run(
        add: Option<Add>,
        get: GetLog,
        set: SetLog,
        present: bool,
        noise: u8,
        count: usize,
        parented: c_uint,
        callbacks: c_uint,
        source_flags: u32,
        filter_flags: u32,
        owns_info_id: bool,
        dying: bool,
        which: usize,
        out: *mut c_long,
        message: *mut c_char,
        message_size: usize,
    ) -> bool;
    fn oracle_sfa_layout(which: usize) -> usize;
    fn obs_source_filter_add_lock(source: *mut c_void);
    fn obs_source_filter_add_count(source: *const c_void) -> usize;
    fn obs_source_filter_add_at(source: *const c_void, index: usize) -> *mut c_void;
    fn obs_source_filter_add_has_parent(filter: *const c_void) -> bool;
    fn obs_source_filter_add_warn_present();
    fn obs_source_filter_add_owns_info_id(source: *const c_void) -> bool;
    fn obs_source_filter_add_output_flags(source: *const c_void) -> u32;
    fn obs_source_filter_add_get_ref(filter: *mut c_void) -> *mut c_void;
    fn obs_source_filter_add_set_parent(filter: *mut c_void, source: *mut c_void);
    fn obs_source_filter_add_set_target(filter: *mut c_void, target: *mut c_void);
    fn obs_source_filter_add_insert_front(source: *mut c_void, filter: *mut c_void);
    fn obs_source_filter_add_unlock(source: *mut c_void);
    fn obs_source_filter_add_signal(source: *mut c_void, filter: *mut c_void);
    fn obs_source_filter_add_log(source: *mut c_void, filter: *mut c_void);
    fn obs_source_filter_add_has_callback(filter: *const c_void) -> bool;
    fn obs_source_filter_add_callback(filter: *mut c_void);
}

/// The filter slots (`SFA_SLOTS`); a chain holds at most one less.
const SLOTS: usize = 8;
const CHAIN: usize = SLOTS - 1;
/// `which` for a NULL filter.
const NULL_FILTER: usize = SLOTS;
/// The event width (`SFA_EVENT_WIDTH`) and the out array's layout.
const EVENT_WIDTH: usize = 5;
const EVENTS: usize = 16;
const COUNT_AFTER: usize = 0;
const ORDER: usize = 1;
const TARGETS: usize = ORDER + SLOTS;
const PARENTS: usize = TARGETS + SLOTS;
const UNCHANGED: usize = PARENTS + SLOTS;
const UNLOCKED: usize = UNCHANGED + 1;
const LOGS: usize = UNCHANGED + 2;
const EVENT_COUNT: usize = UNCHANGED + 3;
const OVERFLOW: usize = UNCHANGED + 4;
const HEAD: usize = UNCHANGED + 5;
const OUT: usize = HEAD + EVENTS * EVENT_WIDTH;
/// Event kinds (`SFA_EV_*`).
const EV_GLOBAL_SIGNAL: c_long = 1;
const EV_SOURCE_SIGNAL: c_long = 2;
const EV_CALLBACK: c_long = 3;
const EV_REF: c_long = 4;
const EV_LOG: c_long = 5;
/// Source ids (`SFA_PARENT`, `SFA_UNTOUCHED`).
const PARENT: c_long = -3;
const UNTOUCHED: c_long = -4;
/// `LOG_WARNING`, `LOG_DEBUG`.
const LOG_WARNING: c_long = 200;
const LOG_DEBUG: c_long = 400;

/// What one call left: the out array and the last log line.
type Seen = (Vec<c_long>, String);

#[derive(Clone, Copy, Debug)]
struct Case {
    present: bool,
    noise: u8,
    count: usize,
    parented: c_uint,
    callbacks: c_uint,
    source_flags: u32,
    filter_flags: u32,
    owns_info_id: bool,
    dying: bool,
    which: usize,
}

fn run(add: Option<Add>, case: Case) -> Seen {
    let mut out: [c_long; OUT] = [0; OUT];
    let mut buffer = [0 as c_char; 128];
    // SAFETY: the isolated child owns the log handler. C builds a live
    // fixture, runs the call, records and frees it, and restores logging;
    // both buffers are as long as passed.
    let built = unsafe {
        oracle_sfa_run(
            add,
            base_get_log_handler,
            base_set_log_handler,
            case.present,
            case.noise,
            case.count,
            case.parented,
            case.callbacks,
            case.source_flags,
            case.filter_flags,
            case.owns_info_id,
            case.dying,
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
fn compare_with(add: Add, case: Case) -> Option<Seen> {
    let c = run(None, case);
    let rust = run(Some(add), case);
    (c == rust).then_some(c)
}

/// Each recorded event's kind and lock state, in order.
fn events(out: &[c_long]) -> Vec<(c_long, c_long)> {
    let count = usize::try_from(out[EVENT_COUNT]).unwrap();
    (0..count)
        .map(|i| (out[HEAD + i * EVENT_WIDTH], out[HEAD + i * EVENT_WIDTH + 4]))
        .collect()
}

fn compare(case: Case) -> Seen {
    let seen = compare_with(obs_source_filter_add, case).unwrap_or_else(|| {
        panic!(
            "{case:?}: C {:?}, Rust {:?}",
            run(None, case),
            run(Some(obs_source_filter_add), case)
        )
    });
    let out = &seen.0;
    // Nothing else written, the lock free, no event lost.
    assert_eq!(
        (out[UNCHANGED], out[UNLOCKED], out[OVERFLOW]),
        (1, 1, 0),
        "{case:?}"
    );
    seen
}

/// Mutants of the export: the real core over a list that breaks one call.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Break {
    NoParent,
    SourceTarget,
    SignalLocked,
    NoCallback,
    NoWarning,
}

struct Mutant {
    source: NonNull<c_void>,
    broken: Break,
}

impl FilterList for Mutant {
    type Source = *mut c_void;
    fn this(&self) -> *mut c_void {
        self.source.as_ptr()
    }
    fn lock(&self) {
        // SAFETY: a live fixture source.
        unsafe { obs_source_filter_add_lock(self.source.as_ptr()) }
    }
    fn count(&self) -> usize {
        // SAFETY: as the export's.
        unsafe { obs_source_filter_add_count(self.source.as_ptr()) }
    }
    fn filter(&self, index: usize) -> *mut c_void {
        if self.broken == Break::SourceTarget {
            return self.source.as_ptr();
        }
        // SAFETY: as the export's.
        unsafe { obs_source_filter_add_at(self.source.as_ptr(), index) }
    }
    fn has_parent(&self, filter: *mut c_void) -> bool {
        // SAFETY: as the export's.
        unsafe { obs_source_filter_add_has_parent(filter) }
    }
    fn warn_present(&self) {
        if self.broken != Break::NoWarning {
            // SAFETY: as the export's.
            unsafe { obs_source_filter_add_warn_present() }
        }
    }
    fn owns_info_id(&self) -> bool {
        // SAFETY: as the export's.
        unsafe { obs_source_filter_add_owns_info_id(self.source.as_ptr()) }
    }
    fn output_flags(&self) -> u32 {
        // SAFETY: as the export's.
        unsafe { obs_source_filter_add_output_flags(self.source.as_ptr()) }
    }
    fn filter_output_flags(&self, filter: *mut c_void) -> u32 {
        // SAFETY: as the export's.
        unsafe { obs_source_filter_add_output_flags(filter) }
    }
    fn get_ref(&self, filter: *mut c_void) -> Option<*mut c_void> {
        // SAFETY: as the export's.
        let referenced = unsafe { obs_source_filter_add_get_ref(filter) };
        (!referenced.is_null()).then_some(referenced)
    }
    fn set_parent(&self, filter: *mut c_void) {
        if self.broken != Break::NoParent {
            // SAFETY: as the export's.
            unsafe { obs_source_filter_add_set_parent(filter, self.source.as_ptr()) }
        }
    }
    fn set_target(&self, filter: *mut c_void, target: *mut c_void) {
        // SAFETY: as the export's.
        unsafe { obs_source_filter_add_set_target(filter, target) }
    }
    fn insert_front(&self, filter: *mut c_void) {
        // SAFETY: as the export's.
        unsafe { obs_source_filter_add_insert_front(self.source.as_ptr(), filter) }
    }
    fn unlock(&self) {
        // SAFETY: as the export's.
        unsafe { obs_source_filter_add_unlock(self.source.as_ptr()) }
    }
    fn signal(&self, filter: *mut c_void) {
        if self.broken == Break::SignalLocked {
            // SAFETY: as the export's, but under the lock.
            unsafe {
                obs_source_filter_add_lock(self.source.as_ptr());
                obs_source_filter_add_signal(self.source.as_ptr(), filter);
                obs_source_filter_add_unlock(self.source.as_ptr());
            }
        } else {
            // SAFETY: as the export's.
            unsafe { obs_source_filter_add_signal(self.source.as_ptr(), filter) }
        }
    }
    fn log_added(&self, filter: *mut c_void) {
        // SAFETY: as the export's.
        unsafe { obs_source_filter_add_log(self.source.as_ptr(), filter) }
    }
    fn has_add_callback(&self, filter: *mut c_void) -> bool {
        // SAFETY: as the export's.
        self.broken != Break::NoCallback && unsafe { obs_source_filter_add_has_callback(filter) }
    }
    fn call_add_callback(&self, filter: *mut c_void) {
        // SAFETY: as the export's.
        unsafe { obs_source_filter_add_callback(filter) }
    }
}

unsafe extern "C" fn mutant_add<const B: u8>(source: *mut c_void, filter: *mut c_void) {
    let (Some(source), false) = (NonNull::new(source), filter.is_null()) else {
        // SAFETY: as the export's.
        return unsafe { obs_source_filter_add(source, filter) };
    };
    let broken = match B {
        0 => Break::NoParent,
        1 => Break::SourceTarget,
        2 => Break::SignalLocked,
        3 => Break::NoCallback,
        _ => Break::NoWarning,
    };
    let mutant = Mutant { source, broken };
    add(Some(&mutant), Some(filter), |_, _| panic!("logged"));
}

fn case(count: usize, which: usize) -> Case {
    Case {
        present: true,
        noise: 0x5a,
        count,
        parented: 0,
        callbacks: 0xff,
        source_flags: VIDEO,
        filter_flags: VIDEO,
        owns_info_id: false,
        dying: false,
        which,
    }
}

/// Each mutant disagrees with the original C somewhere.
fn rejects_mutants() {
    let mutants: [Add; 5] = [
        mutant_add::<0>,
        mutant_add::<1>,
        mutant_add::<2>,
        mutant_add::<3>,
        mutant_add::<4>,
    ];
    for mutant in mutants {
        let caught = (0..SLOTS).any(|which| compare_with(mutant, case(4, which)).is_none());
        assert!(caught, "a filter_add mutant agreed with the original C");
    }
}

const SOURCE_LINE: &str = "obs_source_filter_add: Null 'source' parameter";
const FILTER_LINE: &str = "obs_source_filter_add: Null 'filter' parameter";
const PRESENT_LINE: &str = "Tried to add a filter that was already present on the source";

/// NULL source or filter: one line, nothing else.
fn null_arguments() {
    for (present, which, line) in [
        (false, CHAIN, SOURCE_LINE),
        (false, NULL_FILTER, SOURCE_LINE),
        (true, NULL_FILTER, FILTER_LINE),
    ] {
        let (out, message) = compare(Case {
            present,
            ..case(3, which)
        });
        assert_eq!(message, line);
        assert_eq!(events(&out), [(EV_LOG, 0)]);
        assert_eq!(out[LOGS], 1);
        assert_eq!(out[COUNT_AFTER], 3);
    }
}

/// One grid case, compared and checked against the contract; whether the
/// filter was added.
fn grid_case(case: Case) -> bool {
    let (out, message) = compare(case);
    let seen = events(&out);
    let count = c_long::try_from(case.count).unwrap();
    let listed = case.which < case.count;
    let parented = !listed && case.parented & (1 << case.which) != 0;
    let fits = case.owns_info_id || compatible(case.source_flags, case.filter_flags);
    if listed || parented {
        assert_eq!(seen, [(EV_LOG, 1)], "{case:?}");
        assert_eq!(out[HEAD + 1], LOG_WARNING);
        assert_eq!(message, PRESENT_LINE);
        assert_eq!(out[COUNT_AFTER], count);
        return false;
    }
    if !fits {
        assert!(seen.is_empty(), "{case:?}");
        assert_eq!(out[COUNT_AFTER], count);
        return false;
    }
    if case.dying {
        assert_eq!(seen, [(EV_REF, 1), (EV_LOG, 1)], "{case:?}");
        assert_eq!(out[HEAD + EVENT_WIDTH + 1], LOG_DEBUG);
        assert_eq!(message, FILTER_LINE);
        assert_eq!(out[COUNT_AFTER], count);
        let which = case.which;
        assert_eq!(
            (out[TARGETS + which], out[PARENTS + which]),
            (UNTOUCHED, UNTOUCHED)
        );
        return false;
    }
    let names = ["a", "bb", "", "dddd", "e", "ffffff", "g", "newcomer"];
    assert_eq!(
        message,
        format!(
            "- filter '{}' (f{}) added to source 'parent'",
            names[case.which], case.which
        )
    );
    let mut expected = vec![
        (EV_REF, 1),
        (EV_GLOBAL_SIGNAL, 0),
        (EV_SOURCE_SIGNAL, 0),
        (EV_LOG, 0),
    ];
    if case.callbacks & (1 << case.which) != 0 {
        expected.push((EV_CALLBACK, 0));
    }
    assert_eq!(seen, expected, "{case:?}");
    let which = c_long::try_from(case.which).unwrap();
    assert_eq!(out[COUNT_AFTER], count + 1);
    assert_eq!(out[ORDER], which);
    let target = if case.count == 0 { PARENT } else { 0 };
    assert_eq!(
        (out[TARGETS + case.which], out[PARENTS + case.which]),
        (target, PARENT)
    );
    true
}

/// Every chain of up to 7 filters, every filter of the fixture (listed or
/// not), each listed filter parented elsewhere or not, with and without its
/// callback, over source and filter flags that fit or not, a source that
/// owns its info id or not, and a filter that can be referenced or not
/// (sampled past 4 filters).
fn every_chain() {
    let flags = [
        (VIDEO, VIDEO),
        (VIDEO, AUDIO),
        (AUDIO, AUDIO | ASYNC),
        (VIDEO | AUDIO, VIDEO | ASYNC),
    ];
    let mut grid = 0u64;
    let mut added = 0usize;
    for count in 0..=CHAIN {
        for which in 0..SLOTS {
            for callbacks in [0, c_uint::MAX, 1 << which] {
                for parented in [0, 1 << which, !(1 << which)] {
                    for (source_flags, filter_flags) in flags {
                        for owns_info_id in [false, true] {
                            for dying in [false, true] {
                                grid += 1;
                                if count >= 5 && !obs_c_oracle::parity::keep(grid) {
                                    continue;
                                }
                                let noise = u8::try_from(grid % 256).unwrap().wrapping_mul(37);
                                added += usize::from(grid_case(Case {
                                    present: true,
                                    noise,
                                    count,
                                    parented,
                                    callbacks,
                                    source_flags,
                                    filter_flags,
                                    owns_info_id,
                                    dying,
                                    which,
                                }));
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(added > 0);
}

#[test]
fn filter_add_matches_c() {
    if !crate::output::isolated(concat!(module_path!(), "::filter_add_matches_c")) {
        return;
    }
    // SAFETY: plain reads of the oracle's constants.
    assert_eq!(
        unsafe {
            [
                oracle_sfa_layout(0),
                oracle_sfa_layout(1),
                oracle_sfa_layout(2),
            ]
        },
        [OUT, SLOTS, EVENT_WIDTH]
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
                any::<(bool, u8, c_uint, c_uint, bool, bool)>(),
                (0..8u32, 0..8u32),
                0..=CHAIN,
                0..=NULL_FILTER,
            ),
            |(
                (present, noise, parented, callbacks, owns_info_id, dying),
                (sf, ff),
                count,
                which,
            )| {
                compare(Case {
                    present,
                    noise,
                    count,
                    parented: parented & 0xff,
                    callbacks: callbacks & 0xff,
                    source_flags: sf,
                    filter_flags: ff,
                    owns_info_id,
                    dying,
                    which,
                });
                Ok(())
            },
        )
        .unwrap();
}
