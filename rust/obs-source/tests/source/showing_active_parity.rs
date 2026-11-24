//! Tier 3: the original C predicates and the Rust exports on noise-filled
//! real-header sources, with exact diagnostics. No intentional differences
//! or excluded inputs.
use core::ffi::{c_long, c_void};
use obs_source::ffi::source_showing_active::{obs_source_active, obs_source_showing};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Query = unsafe extern "C" fn(*const c_void) -> bool;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

unsafe extern "C" {
    fn oracle_source_showing_active_check(
        active: Query,
        showing: Query,
        get: GetLog,
        set: SetLog,
        present: bool,
        noise: u8,
        activate_refs: c_long,
        show_refs: c_long,
        active_out: *mut bool,
        showing_out: *mut bool,
    ) -> bool;
}

/// Both answers, after checking both sides agree on them, their logs and
/// every source byte.
fn check_with(
    [active, showing]: [Query; 2],
    present: bool,
    noise: u8,
    [activate_refs, show_refs]: [c_long; 2],
) -> Option<[bool; 2]> {
    let (mut active_out, mut showing_out) = (false, false);
    // SAFETY: the isolated child owns the log handler. C builds live sources,
    // runs both pairs, compares results, logs and bytes, and restores
    // logging.
    let equal = unsafe {
        oracle_source_showing_active_check(
            active,
            showing,
            base_get_log_handler,
            base_set_log_handler,
            present,
            noise,
            activate_refs,
            show_refs,
            &raw mut active_out,
            &raw mut showing_out,
        )
    };
    equal.then_some([active_out, showing_out])
}

const REAL: [Query; 2] = [obs_source_active, obs_source_showing];

fn check(present: bool, noise: u8, refs: [c_long; 2]) -> [bool; 2] {
    check_with(REAL, present, noise, refs)
        .unwrap_or_else(|| panic!("present={present}, noise={noise:#x}, refs={refs:#x?}"))
}

/// Mutants of the shims: each must disagree with the original somewhere.
unsafe extern "C" fn swapped_counter(source: *const c_void) -> bool {
    if source.is_null() {
        // SAFETY: NULL logs under the real name.
        return unsafe { obs_source_active(source) };
    }
    // SAFETY: a live oracle source.
    unsafe { obs_source_showing(source) }
}
unsafe extern "C" fn silent_null(source: *const c_void) -> bool {
    if source.is_null() {
        return false;
    }
    // SAFETY: a live oracle source.
    unsafe { obs_source_active(source) }
}
unsafe extern "C" fn wrong_name(source: *const c_void) -> bool {
    if source.is_null() {
        // SAFETY: NULL answers false as the real export does, but logs
        // obs_source_active's line instead of its own.
        return unsafe { obs_source_active(source) };
    }
    // SAFETY: a live oracle source.
    unsafe { obs_source_showing(source) }
}

fn edges() -> Vec<c_long> {
    let mut values = vec![0, 1, 2, -1, 0x5a5a, c_long::MAX, c_long::MIN];
    values.extend((0..c_long::BITS).map(|bit| 1 << bit));
    values
}

#[test]
fn showing_active_results_and_diagnostics_match_c() {
    if !crate::output::isolated(concat!(
        module_path!(),
        "::showing_active_results_and_diagnostics_match_c"
    )) {
        return;
    }
    let edges = edges();
    for noise in [0, 0x5a, 0xff] {
        // NULL is false whatever the bytes would have held.
        assert_eq!(check(false, noise, [c_long::MAX; 2]), [false; 2]);
        for &activate in &edges {
            for &show in &edges {
                // The expected answers pin the shared result, so a mutation on
                // both sides (or in the oracle) still fails: each predicate is
                // its own counter being nonzero.
                assert_eq!(
                    check(true, noise, [activate, show]),
                    [activate != 0, show != 0]
                );
            }
        }
    }
    let mutants: [[Query; 2]; 3] = [
        [swapped_counter, obs_source_showing],
        [silent_null, obs_source_showing],
        [obs_source_active, wrong_name],
    ];
    for mutant in mutants {
        let caught = [(false, [0, 0]), (true, [1, 0])]
            .into_iter()
            .any(|(present, refs)| check_with(mutant, present, 0x5a, refs).is_none());
        assert!(caught, "a mutant of the shims agreed with the original C");
    }
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(
            &(any::<bool>(), any::<u8>(), any::<[c_long; 2]>()),
            |(present, noise, refs)| {
                let result = check(present, noise, refs);
                let expected = if present {
                    [refs[0] != 0, refs[1] != 0]
                } else {
                    [false; 2]
                };
                prop_assert_eq!(result, expected);
                Ok(())
            },
        )
        .unwrap();
}
