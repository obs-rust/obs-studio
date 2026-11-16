//! Exact original-C parity, real-header layout and full encoder mutation checks.
use core::ffi::c_int;
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::encoder_set_preferred_format::Fields;
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_encoder_set_preferred_format_layout(layout: *mut usize);
    fn oracle_encoder_set_preferred_format_check(kind: c_int, before: u32, format: u32) -> bool;
    fn oracle_encoder_set_preferred_format_null_silent(rust: bool) -> bool;
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 8];
    // SAFETY: the oracle writes exactly eight size_t entries.
    unsafe { oracle_encoder_set_preferred_format_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, kind),
            offset_of!(Fields, format),
            size_of::<c_int>(),
            align_of::<c_int>(),
            size_of::<u32>(),
            align_of::<u32>()
        ]
    );
}

fn check(kind: i32, before: u32, format: u32) {
    // SAFETY: the C probe initializes resource-free real encoders and compares all bytes.
    assert!(unsafe { oracle_encoder_set_preferred_format_check(kind, before, format) });
}

#[test]
fn characterized_edges_and_stored_values() {
    for kind in [0, 1, 2, -1, i32::MIN, i32::MAX] {
        for before in [0, 2, 24, u32::MAX] {
            for format in [0, 1, 2, 24, i32::MAX as u32, 0x8000_0000, u32::MAX] {
                check(kind, before, format);
            }
        }
    }
}

#[test]
fn null_is_silent() {
    if !super::isolated(concat!(module_path!(), "::null_is_silent")) {
        return;
    }
    // SAFETY: each probe restores the process logger in this isolated child.
    unsafe {
        assert!(oracle_encoder_set_preferred_format_null_silent(false));
        assert!(oracle_encoder_set_preferred_format_null_silent(true));
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn random_values_match_c(kind in prop_oneof![Just(0i32), Just(1i32), any::<i32>()], before in any::<u32>(), format in any::<u32>()) {
        check(kind, before, format);
    }
}
