//! Exact state and diagnostic parity, no intentional differences or exclusions.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::output_set_media::{Fields, obs_output_set_media};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Call = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_output_set_media_layout(layout: *mut usize);
    fn oracle_output_set_media_check(
        call: Call,
        get: GetLog,
        set: SetLog,
        flags: u32,
        null_output: bool,
        choices: u32,
    ) -> bool;
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 6];
    // SAFETY: oracle writes exactly six entries to this array.
    unsafe { oracle_output_set_media_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, flags),
            offset_of!(Fields, video),
            offset_of!(Fields, audio),
            offset_of!(Fields, name)
        ]
    );
}

fn check(flags: u32, null_output: bool, choices: u32) {
    // SAFETY: isolated process owns logging. C probe uses real live fixtures,
    // restores the handler, and compares every byte and complete diagnostics.
    unsafe {
        assert!(oracle_output_set_media_check(
            obs_output_set_media,
            base_get_log_handler,
            base_set_log_handler,
            flags,
            null_output,
            choices
        ));
    }
}

#[test]
fn characterized_edges() {
    if !super::isolated(concat!(module_path!(), "::characterized_edges")) {
        return;
    }
    for flags in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, u32::MAX] {
        for choices in 0..4 {
            check(flags, false, choices);
            check(flags, true, choices);
        }
    }
}

#[test]
fn random_flags_match_c() {
    if !super::isolated(concat!(module_path!(), "::random_flags_match_c")) {
        return;
    }
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(
            &(any::<u32>(), any::<bool>(), 0u32..4),
            |(flags, null_output, choices)| {
                check(flags, null_output, choices);
                Ok(())
            },
        )
        .unwrap();
}
