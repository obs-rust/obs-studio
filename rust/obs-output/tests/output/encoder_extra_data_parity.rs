//! Tier 3: original-C parity with no intentional differences.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::encoder_extra_data::{ExtraDataFields, obs_encoder_get_extra_data};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*const c_void, *mut *mut u8, *mut usize) -> bool;
unsafe extern "C" {
    fn oracle_obs_encoder_get_extra_data(
        encoder: *const c_void,
        data: *mut *mut u8,
        size: *mut usize,
    ) -> bool;
    fn oracle_encoder_extra_data_run(getter: Getter, flags: u32, size: usize, report: *mut u64);
    fn oracle_encoder_extra_data_layout(layout: *mut usize);
}

#[test]
fn real_header_layout() {
    let mut actual = [0usize; 4];
    // SAFETY: the oracle writes exactly four size_t entries to this array.
    unsafe { oracle_encoder_extra_data_layout(actual.as_mut_ptr()) };
    assert_eq!(
        actual,
        [
            size_of::<ExtraDataFields>(),
            align_of::<ExtraDataFields>(),
            offset_of!(ExtraDataFields, callback),
            offset_of!(ExtraDataFields, data)
        ]
    );
}

fn check(flags: u32, size: usize) {
    let mut expected = [0u64; 11];
    let mut actual = [0u64; 11];
    // SAFETY: the runner owns live real-header fixtures and tolerant callbacks,
    // and writes eleven u64 entries. The containing test re-execs for log isolation.
    unsafe {
        oracle_encoder_extra_data_run(
            oracle_obs_encoder_get_extra_data,
            flags,
            size,
            expected.as_mut_ptr(),
        );
        oracle_encoder_extra_data_run(obs_encoder_get_extra_data, flags, size, actual.as_mut_ptr());
    }
    assert_eq!(actual, expected, "flags={flags}, size={size}");
    let null = flags & 8 != 0;
    let dispatch = !null && flags & 3 == 3;
    let mutation = dispatch && flags & 128 != 0;
    assert_eq!(
        actual,
        [
            u64::from(dispatch && flags & 64 != 0),
            u64::from(dispatch),
            0,
            u64::from(dispatch),
            u64::from(dispatch),
            if mutation && flags & 16 != 0 {
                if flags & 256 != 0 { 2 } else { 1 }
            } else {
                0
            },
            if mutation && flags & 32 != 0 {
                size as u64
            } else {
                17
            },
            1,
            u64::from(null),
            if null { 400 } else { 0 },
            u64::from(null),
        ]
    );
}

#[test]
fn callback_gates_outputs_and_diagnostics() {
    if !super::isolated(concat!(
        module_path!(),
        "::callback_gates_outputs_and_diagnostics"
    )) {
        return;
    }
    for flags in 0..512 {
        for size in [
            0,
            1,
            usize::MAX / 2,
            usize::MAX / 2 + 1,
            usize::MAX - 1,
            usize::MAX,
        ] {
            check(flags, size);
        }
    }
    let mut runner = proptest::test_runner::TestRunner::new(proptest::test_runner::Config {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..Default::default()
    });
    runner
        .run(&(0u32..512, any::<usize>()), |(flags, size)| {
            check(flags, size);
            Ok(())
        })
        .unwrap();
}
