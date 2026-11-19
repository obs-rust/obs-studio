//! Tier 3 original-C parity for #5208; no intentional behavior differences.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::output_total_bytes::{TotalBytesFields, obs_output_get_total_bytes};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*const c_void) -> u64;
unsafe extern "C" {
    fn oracle_output_total_bytes_layout(layout: *mut usize);
    fn oracle_obs_output_get_total_bytes(output: *const c_void) -> u64;
    fn oracle_output_total_bytes_run(get: Getter, flags: u32, result: u64, report: *mut u64);
}

#[test]
fn real_header_layout() {
    let mut layout = [0usize; 8];
    // SAFETY: the oracle writes exactly eight size_t values.
    unsafe { oracle_output_total_bytes_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<TotalBytesFields>(),
            align_of::<TotalBytesFields>(),
            offset_of!(TotalBytesFields, get_total_bytes),
            offset_of!(TotalBytesFields, delay_active),
            offset_of!(TotalBytesFields, delay_capturing),
            offset_of!(TotalBytesFields, data),
            size_of::<bool>(),
            align_of::<bool>(),
        ]
    );
}

fn run(get: Getter, flags: u32, result: u64) -> [u64; 8] {
    let mut report = [0u64; 8];
    // SAFETY: the runner owns a zeroed real-layout output on its stack, swaps
    // the global log handler only around the call, and writes eight u64s.
    unsafe { oracle_output_total_bytes_run(get, flags, result, report.as_mut_ptr()) };
    report
}

fn check(flags: u32, result: u64) {
    let expected = run(oracle_obs_output_get_total_bytes, flags, result);
    let actual = run(obs_output_get_total_bytes, flags, result);
    assert_eq!(actual, expected, "flags={flags:#x} result={result:#x}");

    // Independent statement of the characterized C behavior.
    let null = flags & 32 != 0;
    let which = flags & 3;
    let blocked = flags & 4 != 0 && flags & 8 == 0;
    let dispatched = !null && which != 0 && !blocked;
    let value = match (dispatched, which) {
        (true, 1) => result,
        (true, _) => !result,
        (false, _) => 0,
    };
    assert_eq!(
        actual,
        [
            value,
            u64::from(dispatched && which == 1),
            u64::from(dispatched && which != 1),
            u64::from(dispatched || flags & 16 != 0),
            1,
            u64::from(null),
            if null { 400 } else { 0 },
            1,
        ],
        "flags={flags:#x} result={result:#x}"
    );
}

#[test]
fn edge_matrix_matches_c() {
    if !super::isolated(concat!(module_path!(), "::edge_matrix_matches_c")) {
        return;
    }
    for flags in 0..64 {
        if flags & 3 == 3 {
            continue;
        }
        for result in [
            0,
            1,
            u64::from(u32::MAX),
            1 << 32,
            0xfedc_ba98_7654_3210,
            u64::MAX,
        ] {
            check(flags, result);
        }
    }
    let mut runner = proptest::test_runner::TestRunner::new(proptest::test_runner::Config {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..Default::default()
    });
    runner
        .run(
            &(
                (0u32..64).prop_filter("callback selector", |f| f & 3 != 3),
                any::<u64>(),
            ),
            |(flags, result)| {
                check(flags, result);
                Ok(())
            },
        )
        .unwrap();
}
