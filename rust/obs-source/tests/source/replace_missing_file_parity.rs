//! Tier 3: original C and Rust on real-header sources, including exact logs.
//! No excluded inputs except a NULL callback with live private data, which
//! crashes the C. Each side runs on its own noise-filled source.
use core::ffi::{c_char, c_void};
use obs_source::ffi::source_replace_missing_file::{Callback, obs_source_replace_missing_file};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Proc = unsafe extern "C" fn(Option<Callback>, *mut c_void, *const c_char, *mut c_void);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

unsafe extern "C" {
    fn oracle_source_replace_missing_file_check(
        proc_: Proc,
        get: GetLog,
        set: SetLog,
        flags: u32,
        noise: u32,
    ) -> bool;
}

fn check(flags: u32, noise: u32) {
    // SAFETY: the isolated child owns the log handler. C supplies live
    // sources and a real callback, compares both sides and restores logging.
    unsafe {
        assert!(
            oracle_source_replace_missing_file_check(
                obs_source_replace_missing_file,
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
fn callback_dispatch_state_and_diagnostics_match_c() {
    if !crate::output::isolated(concat!(
        module_path!(),
        "::callback_dispatch_state_and_diagnostics_match_c"
    )) {
        return;
    }
    for flags in 0..16 {
        for noise in [0, 0xa5, u32::MAX] {
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
