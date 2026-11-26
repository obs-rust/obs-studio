//! Tier 3: the original C `obs_source_filter_set_order()` and the Rust
//! export on noise-filled real-header sources with a real signal handler:
//! the list, every filter's target, the `reorder_filters` signal (count,
//! calldata, lock state, the list it saw), every other byte, the lock and
//! the exact log line.
//!
//! The original calls the filter chain rules by their libobs names, the
//! rules hybrid libobs ships, so every movement, out-of-enum ones included,
//! compares exactly.
use core::ffi::{CStr, c_char, c_int, c_long, c_uint, c_void};
use core::ptr::NonNull;
use obs_source::ffi::source_filter_set_order::obs_source_filter_set_order;
use obs_source::filter_set_order::{FilterChain, set_order};
use obs_source::source_filter_chain::ASYNC;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type SetOrder = unsafe extern "C" fn(*mut c_void, *mut c_void, c_int);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

unsafe extern "C" {
    fn oracle_sfo_run(
        set_order: Option<SetOrder>,
        get: GetLog,
        set: SetLog,
        present: bool,
        noise: u8,
        count: usize,
        async_mask: c_uint,
        which: usize,
        movement: c_int,
        out: *mut c_long,
        message: *mut c_char,
        message_size: usize,
    ) -> bool;
    fn oracle_sfo_layout(which: usize) -> usize;
    fn obs_source_filter_set_order_lock(source: *mut c_void);
    fn obs_source_filter_set_order_count(source: *const c_void) -> usize;
    fn obs_source_filter_set_order_at(source: *const c_void, index: usize) -> *mut c_void;
    fn obs_source_filter_set_order_flags(filter: *const c_void) -> u32;
    fn obs_source_filter_set_order_move(source: *mut c_void, from: usize, to: usize);
    fn obs_source_filter_set_order_target(filter: *mut c_void, target: *mut c_void);
    fn obs_source_filter_set_order_unlock(source: *mut c_void);
    fn obs_source_filter_set_order_signal(source: *mut c_void);
}

/// The most filters a chain gets (`SFO_FILTERS`).
const FILTERS: usize = 7;
/// `which` for the filter that is on no list.
const OUTSIDER: usize = FILTERS;
/// `which` for a NULL filter.
const NULL_FILTER: usize = FILTERS + 1;
/// The length of the oracle's out array (`SFO_OUT_COUNT`).
const OUT: usize = 23;
/// Indexes into it: the `reorder_filters` signals, then five checks that
/// hold on every call (a signal's calldata source, the lock free during it,
/// the list moved before it, no other byte written, the lock free after),
/// the log count and level, and the global signals.
const SIGNALS: usize = 2 * FILTERS;
const SIGNAL_CHECKS: core::ops::Range<usize> = SIGNALS + 1..SIGNALS + 6;
const LOGS: usize = SIGNALS + 6;
const GLOBAL_SIGNALS: usize = SIGNALS + 8;

const MOVEMENTS: [c_int; 9] = [-1, 0, 1, 2, 3, 4, 5, c_int::MIN, c_int::MAX];

/// What one call left: the out array and the log line.
type Seen = (Vec<c_long>, String);

#[derive(Clone, Copy, Debug)]
struct Case {
    present: bool,
    noise: u8,
    count: usize,
    async_mask: c_uint,
    which: usize,
    movement: c_int,
}

fn run(set_order: Option<SetOrder>, case: Case) -> Seen {
    let mut out: [c_long; OUT] = [0; OUT];
    let mut buffer = [0 as c_char; 128];
    // SAFETY: the isolated child owns the log handler. C builds a live
    // fixture, runs the call, records and frees it, and restores logging;
    // both buffers are as long as passed.
    let built = unsafe {
        oracle_sfo_run(
            set_order,
            base_get_log_handler,
            base_set_log_handler,
            case.present,
            case.noise,
            case.count,
            case.async_mask,
            case.which,
            case.movement,
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
fn compare_with(set_order: SetOrder, case: Case) -> Option<Seen> {
    let c = run(None, case);
    let rust = run(Some(set_order), case);
    (c == rust).then_some(c)
}

fn compare(case: Case) -> Seen {
    let seen = compare_with(obs_source_filter_set_order, case).unwrap_or_else(|| {
        panic!(
            "{case:?}: C {:?}, Rust {:?}",
            run(None, case),
            run(Some(obs_source_filter_set_order), case)
        )
    });
    // Nothing else written, the lock free, and any signal from the source,
    // unlocked, after the move.
    assert!(seen.0[SIGNAL_CHECKS].iter().all(|v| *v == 1), "{case:?}");
    // No global signal: obs_source_dosignal(source, NULL, ...).
    assert_eq!(seen.0[GLOBAL_SIGNALS], 0, "{case:?}");
    seen
}

/// Mutants of the export: the real core over a chain that breaks one call.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Break {
    NoRetarget,
    NoMove,
    SignalLocked,
    IgnoreKind,
}

struct Mutant {
    source: NonNull<c_void>,
    broken: Break,
}

impl FilterChain for Mutant {
    type Filter = *mut c_void;
    fn lock(&self) {
        // SAFETY: a live fixture source.
        unsafe { obs_source_filter_set_order_lock(self.source.as_ptr()) }
    }
    fn count(&self) -> usize {
        // SAFETY: as the export's.
        unsafe { obs_source_filter_set_order_count(self.source.as_ptr()) }
    }
    fn filter(&self, index: usize) -> *mut c_void {
        // SAFETY: as the export's.
        unsafe { obs_source_filter_set_order_at(self.source.as_ptr(), index) }
    }
    fn flags(&self, filter: *mut c_void) -> u32 {
        // SAFETY: as the export's.
        let flags = unsafe { obs_source_filter_set_order_flags(filter) };
        if self.broken == Break::IgnoreKind {
            flags & !ASYNC
        } else {
            flags
        }
    }
    fn move_filter(&self, from: usize, to: usize) {
        if self.broken != Break::NoMove {
            // SAFETY: as the export's.
            unsafe { obs_source_filter_set_order_move(self.source.as_ptr(), from, to) }
        }
    }
    fn set_target(&self, filter: *mut c_void, target: Option<*mut c_void>) {
        if self.broken != Break::NoRetarget {
            // SAFETY: as the export's.
            unsafe {
                obs_source_filter_set_order_target(filter, target.unwrap_or(self.source.as_ptr()))
            }
        }
    }
    fn unlock(&self) {
        if self.broken == Break::SignalLocked {
            // SAFETY: as the export's, but under the lock and on every path.
            unsafe { obs_source_filter_set_order_signal(self.source.as_ptr()) }
        }
        // SAFETY: as the export's.
        unsafe { obs_source_filter_set_order_unlock(self.source.as_ptr()) }
    }
    fn signal(&self) {
        if self.broken != Break::SignalLocked {
            // SAFETY: as the export's.
            unsafe { obs_source_filter_set_order_signal(self.source.as_ptr()) }
        }
    }
}

unsafe extern "C" fn mutant<const B: u8>(
    source: *mut c_void,
    filter: *mut c_void,
    movement: c_int,
) {
    let broken = match B {
        0 => Break::NoRetarget,
        1 => Break::NoMove,
        2 => Break::SignalLocked,
        _ => Break::IgnoreKind,
    };
    let (Some(source), false) = (NonNull::new(source), filter.is_null()) else {
        // SAFETY: as the export's.
        return unsafe { obs_source_filter_set_order(source, filter, movement) };
    };
    set_order(
        Some(&Mutant { source, broken }),
        Some(filter),
        movement,
        |_, _| panic!("logged"),
    );
}

/// Each mutant disagrees with the original C somewhere.
fn rejects_mutants() {
    let mutants: [SetOrder; 4] = [mutant::<0>, mutant::<1>, mutant::<2>, mutant::<3>];
    for set_order in mutants {
        let caught = (0..4).any(|movement| {
            (0..4).any(|which| {
                compare_with(
                    set_order,
                    Case {
                        present: true,
                        noise: 0x5a,
                        count: 4,
                        async_mask: 0b0110,
                        which,
                        movement,
                    },
                )
                .is_none()
            })
        });
        assert!(caught, "a set_order mutant agreed with the original C");
    }
}

const SOURCE_LINE: &str = "obs_source_filter_set_order: Null 'source' parameter";
const FILTER_LINE: &str = "obs_source_filter_set_order: Null 'filter' parameter";

#[test]
fn set_order_matches_c() {
    if !crate::output::isolated(concat!(module_path!(), "::set_order_matches_c")) {
        return;
    }
    // SAFETY: plain reads of the oracle's constants.
    assert_eq!(
        unsafe { [oracle_sfo_layout(0), oracle_sfo_layout(1)] },
        [OUT, FILTERS]
    );

    // NULL source or filter: one line, nothing else.
    for movement in MOVEMENTS {
        for (present, which, line) in [
            (false, 0, SOURCE_LINE),
            (false, NULL_FILTER, SOURCE_LINE),
            (true, NULL_FILTER, FILTER_LINE),
        ] {
            let case = Case {
                present,
                noise: 0x5a,
                count: 3,
                async_mask: 0b010,
                which,
                movement,
            };
            let (out, message) = compare(case);
            assert_eq!(message, line);
            assert_eq!(out[LOGS], 1);
            assert_eq!(out[SIGNALS], 0);
        }
    }

    // Every chain of up to 7 filters with every async pattern, every
    // listed filter and one that is not listed, and every movement.
    let mut grid = 0u64;
    let mut moves = 0usize;
    for count in 0..=FILTERS {
        for async_mask in 0..1 << count {
            for which in (0..count).chain([OUTSIDER]) {
                for movement in MOVEMENTS {
                    grid += 1;
                    if count >= 5 && !obs_c_oracle::parity::keep(grid) {
                        continue;
                    }
                    let noise = u8::try_from(grid % 256).unwrap().wrapping_mul(37);
                    let case = Case {
                        present: true,
                        noise,
                        count,
                        async_mask,
                        which,
                        movement,
                    };
                    let (out, message) = compare(case);
                    assert_eq!((out[LOGS], message.as_str()), (0, ""), "{case:?}");
                    moves += usize::try_from(out[SIGNALS]).unwrap();
                    if which == OUTSIDER || !(0..4).contains(&movement) {
                        assert_eq!(out[SIGNALS], 0, "{case:?}");
                    }
                }
            }
        }
    }
    assert!(moves > 0);

    rejects_mutants();
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(
            &(
                any::<(bool, u8, c_uint)>(),
                prop_oneof![-1..6, any::<c_int>()],
                0..=FILTERS,
                0..=NULL_FILTER,
            ),
            |((present, noise, async_mask), movement, count, which)| {
                compare(Case {
                    present,
                    noise,
                    count,
                    async_mask: async_mask & 0x7f,
                    which,
                    movement,
                });
                Ok(())
            },
        )
        .unwrap();
}
