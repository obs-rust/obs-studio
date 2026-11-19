//! Tier 3: untouched C body and real private projection. No intentional differences.
use core::ffi::{c_char, c_void};
use obs_encoder::ffi::get_name::obs_encoder_get_name;
use proptest::prelude::*;
use std::ffi::CString;

type Getter = unsafe extern "C" fn(*const c_void) -> *const c_char;
unsafe extern "C" {
    fn oracle_encoder_get_name_probe(
        get: Getter,
        name: *const c_char,
        renamed: *const c_char,
        null_name: bool,
    ) -> bool;
}

fn check(name: &[u8], renamed: &[u8], null_name: bool) {
    let name = CString::new(name).unwrap();
    let renamed = CString::new(renamed).unwrap();
    // SAFETY: C borrows the bounded strings and passes live real-layout encoders
    // (or NULL) to both getters. The isolated process owns its log handler.
    unsafe {
        assert!(oracle_encoder_get_name_probe(
            obs_encoder_get_name,
            name.as_ptr(),
            renamed.as_ptr(),
            null_name
        ));
    }
}

#[test]
fn original_c_characterization_and_edges() {
    if !super::isolated(concat!(
        module_path!(),
        "::original_c_characterization_and_edges"
    )) {
        return;
    }
    for name in [
        b"get name".as_slice(),
        b"__unnamed0017",
        b"before",
        b"",
        b"%s-%n",
        b"\xff\x80",
    ] {
        for null_name in [false, true] {
            check(name, b"after", null_name);
        }
    }
}

#[test]
fn randomized_original_c_parity() {
    if !super::isolated(concat!(module_path!(), "::randomized_original_c_parity")) {
        return;
    }
    use proptest::test_runner::{Config, TestRunner};
    let mut runner = TestRunner::new(Config {
        cases: obs_c_oracle::parity::cases(128),
        failure_persistence: None,
        ..Config::default()
    });
    runner
        .run(
            &(
                proptest::collection::vec(1_u8..=255, 0..128),
                proptest::collection::vec(1_u8..=255, 0..128),
                any::<bool>(),
            ),
            |(name, renamed, null_name)| {
                check(&name, &renamed, null_name);
                Ok(())
            },
        )
        .unwrap();
}
