//! Real-header layout and exact differential checks for #3900. No intentional differences.
#![allow(unsafe_code)]
use core::ffi::{c_char, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::encoder_last_error::{LastErrorFields, obs_encoder_get_last_error};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_obs_encoder_get_last_error(encoder: *mut c_void) -> *const c_char;
    fn oracle_encoder_last_error_layout(layout: *mut usize);
    fn oracle_encoder_last_error_new(message: *const c_char) -> *mut c_void;
    fn oracle_encoder_last_error_unchanged(encoder: *mut c_void) -> bool;
    fn oracle_encoder_last_error_free(encoder: *mut c_void);
    fn oracle_encoder_last_error_null_log(
        getter: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    ) -> bool;
}

struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: this fixture was allocated by the matching oracle helper.
        unsafe { oracle_encoder_last_error_free(self.0) };
    }
}

fn fixture(message: *const c_char) -> Fixture {
    // SAFETY: the helper stores the pointer without reading or freeing its pointee.
    let encoder = unsafe { oracle_encoder_last_error_new(message) };
    assert!(!encoder.is_null(), "allocate real-header encoder fixture");
    Fixture(encoder)
}

fn check(present: bool, bytes: &[u8]) {
    let mut bytes = bytes.to_vec();
    bytes.push(0);
    let pointer = if present {
        bytes.as_ptr().cast()
    } else {
        std::ptr::null()
    };
    let encoder = fixture(pointer);
    let other_pointer = c"independent".as_ptr();
    let other = fixture(other_pointer);
    for (encoder, expected) in [(&encoder, pointer), (&other, other_pointer)] {
        // SAFETY: both fixtures and their borrowed message storage live through the calls.
        unsafe {
            let c = oracle_obs_encoder_get_last_error(encoder.0);
            let rust = obs_encoder_get_last_error(encoder.0);
            assert_eq!(c, expected);
            assert_eq!(rust, c);
            assert!(oracle_encoder_last_error_unchanged(encoder.0));
        }
    }
}

#[test]
fn layout_matches_real_header() {
    let mut c = [usize::MAX; 3];
    // SAFETY: the helper writes exactly three size_t values.
    unsafe { oracle_encoder_last_error_layout(c.as_mut_ptr()) };
    assert_eq!(
        c,
        [
            size_of::<LastErrorFields>(),
            align_of::<LastErrorFields>(),
            offset_of!(LastErrorFields, message)
        ]
    );
}

#[test]
fn exact_null_diagnostic() {
    if !super::isolated(concat!(module_path!(), "::exact_null_diagnostic")) {
        return;
    }
    // SAFETY: the isolated child exclusively owns the process-wide log callback.
    unsafe {
        assert!(oracle_encoder_last_error_null_log(
            oracle_obs_encoder_get_last_error
        ));
        assert!(oracle_encoder_last_error_null_log(
            obs_encoder_get_last_error
        ));
    }
}

#[test]
fn characterized_edges() {
    check(false, b"");
    for bytes in [b"".as_slice(), b"message", &[b'a', 0xff, b'b'], b"a\0b"] {
        check(true, bytes);
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn random_messages_match_c(present in any::<bool>(), bytes in prop::collection::vec(any::<u8>(), 0..=256)) {
        check(present, &bytes);
    }
}
