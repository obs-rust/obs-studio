//! Exact original-C value, diagnostic and state parity; no exclusions.
use core::ffi::{c_char, c_int, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::encoder_frame_rate_divisor::{self as rust, DivisorFields};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_encoder_divisor_layout(layout: *mut usize);
    fn oracle_encoder_divisor_new(kind: c_int, divisor: u32, name: *const c_char) -> *mut c_void;
    fn oracle_encoder_divisor_free(encoder: *mut c_void);
    fn oracle_encoder_divisor_unchanged(encoder: *mut c_void) -> bool;
    fn oracle_encoder_divisor_probe(
        encoder: *const c_void,
        getter: unsafe extern "C" fn(*const c_void) -> u32,
    ) -> bool;
}

#[test]
fn real_header_view_and_enum_layout() {
    let mut c = [0; 9];
    // SAFETY: the oracle writes exactly nine size_t values.
    unsafe { oracle_encoder_divisor_layout(c.as_mut_ptr()) };
    assert_eq!(
        c,
        [
            size_of::<DivisorFields>(),
            align_of::<DivisorFields>(),
            offset_of!(DivisorFields, kind),
            offset_of!(DivisorFields, divisor),
            offset_of!(DivisorFields, name),
            size_of::<c_int>(),
            align_of::<c_int>(),
            0,
            1
        ]
    );
}

struct Encoder(*mut c_void);
impl Drop for Encoder {
    fn drop(&mut self) {
        // SAFETY: the fixture owns this allocation and frees it exactly once.
        unsafe { oracle_encoder_divisor_free(self.0) };
    }
}
fn check(kind: i32, divisor: u32, name: usize) {
    let names = [c"", c"divisor-test", c"other-\u{e9}"];
    // SAFETY: the fixture borrows a static string and initializes real C fields.
    let encoder =
        Encoder(unsafe { oracle_encoder_divisor_new(kind, divisor, names[name].as_ptr()) });
    assert!(!encoder.0.is_null());
    // SAFETY: the real-layout fixture remains live/exclusive and the callback has the C ABI.
    unsafe {
        assert!(oracle_encoder_divisor_probe(
            encoder.0,
            rust::obs_encoder_get_frame_rate_divisor
        ));
        assert!(oracle_encoder_divisor_unchanged(encoder.0));
    }
}

#[test]
fn characterized_edges_and_logs() {
    if !super::isolated(concat!(module_path!(), "::characterized_edges_and_logs")) {
        return;
    }
    // SAFETY: this null probe saves/restores the logger inside an isolated process.
    assert!(unsafe {
        oracle_encoder_divisor_probe(core::ptr::null(), rust::obs_encoder_get_frame_rate_divisor)
    });
    for kind in [0, 1] {
        for divisor in [0, 1, 2, 0x8000_0000, u32::MAX] {
            for name in 0..3 {
                check(kind, divisor, name);
            }
        }
    }
}

#[test]
fn randomized_values_and_logs() {
    if !super::isolated(concat!(module_path!(), "::randomized_values_and_logs")) {
        return;
    }
    let config = ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    };
    proptest::test_runner::TestRunner::new(config)
        .run(
            &(0i32..2, any::<u32>(), 0usize..3),
            |(kind, divisor, name)| {
                check(kind, divisor, name);
                Ok(())
            },
        )
        .unwrap();
}
