//! Tier 3: the original C query and the Rust export on real-header sources,
//! with exact diagnostics. No intentional differences or excluded inputs.
use obs_source::ffi::source_configurable::obs_source_configurable;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Query = unsafe extern "C" fn(*const core::ffi::c_void) -> bool;
type GetLog = unsafe extern "C" fn(*mut *mut core::ffi::c_void, *mut *mut core::ffi::c_void);
type SetLog = unsafe extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void);

unsafe extern "C" {
    fn oracle_source_configurable_check(
        query: Query,
        get: GetLog,
        set: SetLog,
        flags: u32,
        noise: u32,
        answer: *mut bool,
    ) -> bool;
}

/// The answer both sides gave, after checking they agree.
fn check(flags: u32, noise: u32) -> bool {
    let mut answer = false;
    // SAFETY: the isolated child owns the log handler. C builds live sources,
    // runs both queries, compares logs, state and calls, and restores logging.
    unsafe {
        assert!(
            oracle_source_configurable_check(
                obs_source_configurable,
                base_get_log_handler,
                base_set_log_handler,
                flags,
                noise,
                &raw mut answer,
            ),
            "flags={flags:#x}, noise={noise:#x}",
        );
    }
    answer
}

#[test]
fn configurable_answers_and_diagnostics_match_c() {
    if !crate::output::isolated(concat!(
        module_path!(),
        "::configurable_answers_and_diagnostics_match_c"
    )) {
        return;
    }
    for flags in 0..16 {
        let present = flags & 1 != 0;
        let data = flags & 2 != 0;
        let callback = flags & 12 != 0;
        for noise in [0, 1, 0x5a5a_5a5a, u32::MAX] {
            // The expected truth table pins the shared answer, so a mutation
            // on both sides (or in the oracle) still fails.
            assert_eq!(
                check(flags, noise),
                present && data && callback,
                "flags={flags:#x}"
            );
        }
    }
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(&(0u32..16, any::<u32>()), |(flags, noise)| {
            check(flags, noise);
            Ok(())
        })
        .unwrap();
}
