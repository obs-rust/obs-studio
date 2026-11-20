//! Tier 3: the original C query and the Rust export on noise-filled
//! real-header sources, with exact diagnostics. No intentional differences
//! or excluded inputs.
use obs_source::ffi::source_output_flags::obs_source_get_output_flags;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Query = unsafe extern "C" fn(*const core::ffi::c_void) -> u32;
type GetLog = unsafe extern "C" fn(*mut *mut core::ffi::c_void, *mut *mut core::ffi::c_void);
type SetLog = unsafe extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void);

unsafe extern "C" {
    fn oracle_source_output_flags_check(
        query: Query,
        get: GetLog,
        set: SetLog,
        present: bool,
        noise: u8,
        info_flags: u32,
        instance_flags: u32,
        default_flags: u32,
        result: *mut u32,
    ) -> bool;
}

/// The flags both sides returned, after checking they agree.
fn check_with(query: Query, present: bool, noise: u8, flags: [u32; 3]) -> Option<u32> {
    let mut result = 0;
    let [info, instance, default] = flags;
    // SAFETY: the isolated child owns the log handler. C builds live sources,
    // runs both queries, compares results, logs and bytes, and restores
    // logging.
    let equal = unsafe {
        oracle_source_output_flags_check(
            query,
            base_get_log_handler,
            base_set_log_handler,
            present,
            noise,
            info,
            instance,
            default,
            &raw mut result,
        )
    };
    equal.then_some(result)
}

fn check(present: bool, noise: u8, flags: [u32; 3]) -> u32 {
    check_with(obs_source_get_output_flags, present, noise, flags)
        .unwrap_or_else(|| panic!("present={present}, noise={noise:#x}, flags={flags:#x?}"))
}

/// Mutants of the shim: each must disagree with the original somewhere.
unsafe extern "C" fn altered_value(source: *const core::ffi::c_void) -> u32 {
    // SAFETY: NULL or a live oracle source; NULL logs as the real export does.
    let flags = unsafe { obs_source_get_output_flags(source) };
    if source.is_null() { flags } else { flags ^ 1 }
}
unsafe extern "C" fn silent_null(source: *const core::ffi::c_void) -> u32 {
    if source.is_null() {
        return 0;
    }
    // SAFETY: a live oracle source.
    unsafe { obs_source_get_output_flags(source) }
}

#[test]
fn output_flags_results_and_diagnostics_match_c() {
    if !crate::output::isolated(concat!(
        module_path!(),
        "::output_flags_results_and_diagnostics_match_c"
    )) {
        return;
    }
    let edges = [0, 1, 0x8000_0000, 0x5555_5555, 0xaaaa_aaaa, u32::MAX];
    for noise in [0, 0x5a, 0xff] {
        // NULL returns 0 whatever the bytes would have held.
        assert_eq!(check(false, noise, [u32::MAX; 3]), 0);
        for info in edges.into_iter().chain((0..32).map(|bit| 1u32 << bit)) {
            for other in edges {
                // The expected value pins the shared answer, so a mutation on
                // both sides (or in the oracle) still fails: only the info
                // flags count, never the instance or default flags.
                assert_eq!(check(true, noise, [info, other, !other]), info);
            }
        }
    }
    for mutant in [altered_value as Query, silent_null] {
        let caught = [false, true]
            .into_iter()
            .any(|present| check_with(mutant, present, 0x5a, [6, 9, 12]).is_none());
        assert!(caught, "a mutant of the shim agreed with the original C");
    }
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(
            &(any::<bool>(), any::<u8>(), any::<[u32; 3]>()),
            |(present, noise, flags)| {
                let result = check(present, noise, flags);
                prop_assert_eq!(result, if present { flags[0] } else { 0 });
                Ok(())
            },
        )
        .unwrap();
}
