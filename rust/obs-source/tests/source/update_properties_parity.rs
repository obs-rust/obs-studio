//! Original-C parity with real private layouts and synchronous signal callbacks.
//! No intentional differences or excluded inputs. At most sixteen calls per case.
use core::ffi::c_void;
use obs_source::ffi::source_update_properties::obs_source_update_properties;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Entry = unsafe extern "C" fn(*mut c_void);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_obs_source_update_properties(source: *mut c_void);
    fn oracle_source_update_properties_run(
        entry: Entry,
        private_source: bool,
        count: u32,
        reenter: bool,
        mutate: bool,
        trace: *mut u32,
    ) -> bool;
    fn oracle_source_update_properties_null_logs(entry: Entry, get: GetLog, set: SetLog) -> bool;
}

fn compare(private_source: bool, count: u32, reenter: bool, mutate: bool) {
    let mut rust = [0; 16];
    let mut original = [0; 16];
    // SAFETY: the C fixture owns the real source, settings token and both live
    // signal handlers. Both output arrays hold the bounded count of entries.
    unsafe {
        assert!(oracle_source_update_properties_run(
            obs_source_update_properties,
            private_source,
            count,
            reenter,
            mutate,
            rust.as_mut_ptr(),
        ));
        assert!(oracle_source_update_properties_run(
            oracle_obs_source_update_properties,
            private_source,
            count,
            reenter,
            mutate,
            original.as_mut_ptr(),
        ));
    }
    assert_eq!(rust, original);
}

#[test]
fn original_callbacks_repeats_payload_mutation_and_reentrancy() {
    if !crate::output::isolated(concat!(
        module_path!(),
        "::original_callbacks_repeats_payload_mutation_and_reentrancy"
    )) {
        return;
    }
    for private in [false, true] {
        for reenter in [false, true] {
            for mutate in [false, true] {
                for count in [0, 1, 3, 16] {
                    compare(private, count, reenter, mutate);
                }
            }
        }
    }
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(64),
        ..ProptestConfig::default()
    });
    runner
        .run(
            &(any::<bool>(), 0_u32..=16, any::<bool>(), any::<bool>()),
            |(private, count, reenter, mutate)| {
                compare(private, count, reenter, mutate);
                Ok(())
            },
        )
        .unwrap();
}

#[test]
fn null_diagnostic_matches_original() {
    if !crate::output::isolated(concat!(
        module_path!(),
        "::null_diagnostic_matches_original"
    )) {
        return;
    }
    // SAFETY: the isolated child owns the process log callback; the C probe
    // restores it. The setter/getter use the base shim's opaque callback ABI.
    unsafe {
        assert!(oracle_source_update_properties_null_logs(
            obs_source_update_properties,
            base_get_log_handler,
            base_set_log_handler,
        ));
    }
}
