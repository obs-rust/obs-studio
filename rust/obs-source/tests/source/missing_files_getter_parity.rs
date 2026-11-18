//! Tier 3: original C and Rust on real-header sources, including exact logs.
//! No intentional differences or excluded inputs. Every allocation is destroyed.
use core::ffi::c_void;
use obs_source::ffi::source_missing_files_getter::{Fields, obs_source_get_missing_files};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*const c_void) -> *mut c_void;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

unsafe extern "C" {
    fn oracle_source_missing_files_layout(out: *mut usize);
    fn oracle_source_missing_files_check(
        getter: Getter,
        get: GetLog,
        set: SetLog,
        flags: u32,
        noise: u32,
    ) -> bool;
}

#[test]
fn real_header_snapshot_layout() {
    let mut layout = [0; 4];
    // SAFETY: the oracle writes exactly four size/offset measurements.
    unsafe { oracle_source_missing_files_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            core::mem::offset_of!(Fields, data),
            core::mem::offset_of!(Fields, callback),
        ]
    );
}

fn check(flags: u32, noise: u32) {
    // SAFETY: the isolated child owns the log handler. C supplies live sources,
    // validates both getters and callback state, frees lists and restores logging.
    unsafe {
        assert!(
            oracle_source_missing_files_check(
                obs_source_get_missing_files,
                base_get_log_handler,
                base_set_log_handler,
                flags,
                noise,
            ),
            "flags={flags}, noise={noise}",
        );
    }
}

#[test]
fn callback_fallback_state_and_diagnostics_match_c() {
    if !crate::output::isolated(concat!(
        module_path!(),
        "::callback_fallback_state_and_diagnostics_match_c"
    )) {
        return;
    }
    for flags in 0..16 {
        for noise in [0, 1, u32::MAX] {
            check(flags, noise);
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
