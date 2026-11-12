//! Exact C differential coverage, real-header layout and complete encoder-byte checks.
//! No intentional differences or excluded inputs; sequences allocate at most 32 entries.
use core::ffi::{c_int, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::encoder_preferred_color_setters::{
    Fields, obs_encoder_set_preferred_color_space, obs_encoder_set_preferred_range,
};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_encoder_preferred_color_setters_layout(layout: *mut usize);
    fn oracle_encoder_preferred_color_setters_check(
        kind: c_int,
        space: u32,
        range: u32,
        values: *const u32,
        range_writes: *const bool,
        count: usize,
    ) -> bool;
    fn oracle_obs_encoder_set_preferred_color_space(encoder: *mut c_void, space: u32);
    fn oracle_obs_encoder_set_preferred_range(encoder: *mut c_void, range: u32);
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 11];
    // SAFETY: the oracle writes exactly eleven size_t entries to this array.
    unsafe { oracle_encoder_preferred_color_setters_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, kind),
            offset_of!(Fields, space),
            offset_of!(Fields, range),
            size_of::<c_int>(),
            align_of::<c_int>(),
            size_of::<u32>(),
            align_of::<u32>(),
            size_of::<u32>(),
            align_of::<u32>(),
        ]
    );
}

fn check(kind: i32, space: u32, range: u32, steps: &[(bool, u32)]) {
    let range_writes: Vec<bool> = steps.iter().map(|s| s.0).collect();
    let values: Vec<u32> = steps.iter().map(|s| s.1).collect();
    // SAFETY: both arrays have count entries; the probe owns resource-free real encoders
    // and checks member addresses plus every encoder byte after each repeated call.
    assert!(
        unsafe {
            oracle_encoder_preferred_color_setters_check(
                kind,
                space,
                range,
                values.as_ptr(),
                range_writes.as_ptr(),
                steps.len(),
            )
        },
        "kind={kind}, space={space:#x}, range={range:#x}, steps={steps:?}"
    );
}

#[test]
fn null_noops_before_field_access() {
    for value in [0, 1, 2, 3, 4, 5, 0x5a, 0x8000_0000, u32::MAX] {
        // SAFETY: both original setters accept NULL; Rust must guard before its adapter.
        unsafe {
            oracle_obs_encoder_set_preferred_color_space(core::ptr::null_mut(), value);
            oracle_obs_encoder_set_preferred_range(core::ptr::null_mut(), value);
            obs_encoder_set_preferred_color_space(core::ptr::null_mut(), value);
            obs_encoder_set_preferred_range(core::ptr::null_mut(), value);
        }
    }
}

#[test]
fn characterized_cross_product_and_exact_enum_edges() {
    let values = [
        0,
        1,
        2,
        3,
        4,
        5,
        0x5a,
        i32::MAX as u32,
        0x8000_0000,
        u32::MAX,
    ];
    for kind in [0, 1, 2, -1, i32::MIN, i32::MAX] {
        for space in values {
            for range in values {
                for value in values {
                    check(kind, space, range, &[(false, value), (true, value)]);
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn random_sequences_match_c(
        kind in prop_oneof![Just(0i32), Just(1i32), any::<i32>()],
        space in any::<u32>(), range in any::<u32>(),
        steps in prop::collection::vec((any::<bool>(), any::<u32>()), 0..=32),
    ) {
        check(kind, space, range, &steps);
    }
}
