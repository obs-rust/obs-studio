//! Tier 3: exact original-C dispatch/state parity; no intentional differences.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::encoder_priming_samples::{
    PrimingSamplesFields, obs_encoder_get_priming_samples,
};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*const c_void) -> u32;
unsafe extern "C" {
    fn oracle_obs_encoder_get_priming_samples(encoder: *const c_void) -> u32;
    fn oracle_encoder_priming_samples_run(
        getter: Getter,
        flags: u32,
        result: u32,
        token: u32,
        report: *mut u64,
    );
    fn oracle_encoder_priming_samples_layout(layout: *mut usize);
}

#[test]
fn real_header_layout() {
    let mut actual = [0usize; 4];
    // SAFETY: the real-header oracle writes exactly four size_t entries.
    unsafe { oracle_encoder_priming_samples_layout(actual.as_mut_ptr()) };
    assert_eq!(
        actual,
        [
            size_of::<PrimingSamplesFields>(),
            align_of::<PrimingSamplesFields>(),
            offset_of!(PrimingSamplesFields, callback),
            offset_of!(PrimingSamplesFields, data),
        ]
    );
}

fn check(flags: u32, result: u32, token: u32) {
    let mut expected = [0u64; 8];
    let mut actual = [0u64; 8];
    // SAFETY: each runner provides a live real-header encoder and callback/data
    // pair, uses thread-local callback state, and writes eight u64 report entries.
    unsafe {
        oracle_encoder_priming_samples_run(
            oracle_obs_encoder_get_priming_samples,
            flags,
            result,
            token,
            expected.as_mut_ptr(),
        );
        oracle_encoder_priming_samples_run(
            obs_encoder_get_priming_samples,
            flags,
            result,
            token,
            actual.as_mut_ptr(),
        );
    }
    assert_eq!(
        actual, expected,
        "flags={flags}, result={result}, token={token}"
    );
    assert_eq!(
        actual,
        [
            if flags & 1 != 0 { u64::from(result) } else { 0 },
            u64::from(flags & 1),
            0,
            1,
            1,
            1,
            u64::from(token),
            1,
        ]
    );
}

#[test]
fn installed_callback_data_and_full_width_result_parity() {
    // Always cover every combination and the original C characterization's edges.
    for flags in 0..8 {
        for result in [0, 1, 1024, 0x8000_0000, u32::MAX] {
            check(flags, result, 456);
        }
    }
    let mut runner = proptest::test_runner::TestRunner::new(proptest::test_runner::Config {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..Default::default()
    });
    runner
        .run(
            &(0u32..8, any::<u32>(), any::<u32>()),
            |(flags, result, token)| {
                check(flags, result, token);
                Ok(())
            },
        )
        .unwrap();
}
