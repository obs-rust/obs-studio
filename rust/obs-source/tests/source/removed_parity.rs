//! Tier 3: the original C predicate and the Rust export on noise-filled
//! real-header sources, with exact diagnostics. No intentional differences
//! or excluded inputs.
use core::ffi::c_void;
use obs_source::ffi::source_removed::obs_source_removed;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Query = unsafe extern "C" fn(*const c_void) -> bool;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

unsafe extern "C" {
    fn oracle_source_removed_check(
        query: Query,
        get: GetLog,
        set: SetLog,
        present: bool,
        noise: u8,
        removed: bool,
        answer: *mut bool,
    ) -> bool;
}

/// The answer, after checking both sides agree on it, their logs and every
/// source byte.
fn check_with(query: Query, present: bool, noise: u8, removed: bool) -> Option<bool> {
    let mut answer = false;
    // SAFETY: the isolated child owns the log handler. C builds live sources,
    // runs both predicates, compares results, logs and bytes, and restores
    // logging.
    let equal = unsafe {
        oracle_source_removed_check(
            query,
            base_get_log_handler,
            base_set_log_handler,
            present,
            noise,
            removed,
            &raw mut answer,
        )
    };
    equal.then_some(answer)
}

fn check(present: bool, noise: u8, removed: bool) -> bool {
    check_with(obs_source_removed, present, noise, removed)
        .unwrap_or_else(|| panic!("present={present}, noise={noise:#x}, removed={removed}"))
}

/// Mutants of the shim: each must disagree with the original somewhere.
unsafe extern "C" fn null_is_kept(source: *const c_void) -> bool {
    if source.is_null() {
        // SAFETY: NULL logs as the real export does, then answers false.
        unsafe { obs_source_removed(source) };
        return false;
    }
    // SAFETY: a live oracle source.
    unsafe { obs_source_removed(source) }
}
unsafe extern "C" fn silent_null(source: *const c_void) -> bool {
    if source.is_null() {
        return true;
    }
    // SAFETY: a live oracle source.
    unsafe { obs_source_removed(source) }
}
unsafe extern "C" fn negated(source: *const c_void) -> bool {
    // SAFETY: NULL or a live oracle source.
    let answer = unsafe { obs_source_removed(source) };
    if source.is_null() { answer } else { !answer }
}

#[test]
fn removed_result_and_diagnostic_match_c() {
    if !crate::output::isolated(concat!(
        module_path!(),
        "::removed_result_and_diagnostic_match_c"
    )) {
        return;
    }
    for noise in [0, 0x5a, 0xff] {
        // NULL is true whatever the bytes would have held.
        assert!(check(false, noise, false));
        for removed in [false, true] {
            // The expected answer pins the shared result, so a mutation on
            // both sides (or in the oracle) still fails: it is the flag.
            assert_eq!(check(true, noise, removed), removed);
        }
    }
    let mutants: [Query; 3] = [null_is_kept, silent_null, negated];
    for mutant in mutants {
        let caught = [(false, false), (true, false), (true, true)]
            .into_iter()
            .any(|(present, removed)| check_with(mutant, present, 0x5a, removed).is_none());
        assert!(caught, "a mutant of the shim agreed with the original C");
    }
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(
            &(any::<bool>(), any::<u8>(), any::<bool>()),
            |(present, noise, removed)| {
                prop_assert_eq!(check(present, noise, removed), !present || removed);
                Ok(())
            },
        )
        .unwrap();
}
