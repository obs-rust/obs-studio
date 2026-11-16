//! Exact parity for defined C inputs. Messages aliasing the old allocation are
//! excluded: the original C frees it before reading the replacement string.
#![allow(unsafe_code)]
use core::ffi::{CStr, c_char, c_void};
use obs_encoder::ffi::last_error::{obs_encoder_dup_last_error, obs_encoder_set_last_error};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_obs_encoder_set_last_error(output: *mut c_void, message: *const c_char);
    fn oracle_encoder_error_new() -> *mut c_void;
    fn oracle_encoder_error_read(output: *mut c_void) -> *const c_char;
    fn oracle_obs_encoder_dup_last_error(output: *mut c_void) -> *mut c_char;
    fn oracle_encoder_error_free(output: *mut c_void);
    fn oracle_encoder_error_null_log(
        setter: unsafe extern "C" fn(*mut c_void, *const c_char),
    ) -> bool;
}
struct Fixture(*mut c_void);
impl Fixture {
    fn new() -> Self {
        // SAFETY: allocates a zeroed real-header output, without runtime resources.
        let output = unsafe { oracle_encoder_error_new() };
        assert!(!output.is_null());
        Self(output)
    }
    fn message(&self) -> Option<Vec<u8>> {
        // SAFETY: live fixture; setter stores NULL or an owned NUL-terminated copy.
        unsafe {
            let message = oracle_encoder_error_read(self.0);
            (!message.is_null()).then(|| CStr::from_ptr(message).to_bytes().to_vec())
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: matching helper frees both the error with bfree and the fixture.
        unsafe { oracle_encoder_error_free(self.0) };
    }
}
fn check(messages: &[Option<Vec<u8>>]) {
    let c = Fixture::new();
    let rust = Fixture::new();
    assert_eq!(rust.message(), None);
    for message in messages {
        let bytes = message.as_ref().map(|bytes| {
            let mut bytes = bytes.clone();
            bytes.push(0);
            bytes
        });
        let pointer = bytes
            .as_ref()
            .map_or(core::ptr::null(), |v| v.as_ptr().cast());
        // SAFETY: independent message storage and exclusively accessed real-header fixtures.
        unsafe {
            oracle_obs_encoder_set_last_error(c.0, pointer);
            obs_encoder_set_last_error(rust.0, pointer);
            let expected = bytes
                .as_ref()
                .map(|v| v[..v.iter().position(|b| *b == 0).unwrap()].to_vec());
            assert_eq!(c.message(), expected);
            assert_eq!(rust.message(), c.message());
            for (handle, dup) in [
                (
                    c.0,
                    oracle_obs_encoder_dup_last_error
                        as unsafe extern "C" fn(*mut c_void) -> *mut c_char,
                ),
                (rust.0, obs_encoder_dup_last_error),
            ] {
                let first = dup(handle);
                let second = dup(handle);
                if let Some(expected) = &expected {
                    assert!(!first.is_null());
                    assert!(!second.is_null());
                    assert_ne!(first, second);
                    assert_ne!(first.cast_const(), oracle_encoder_error_read(handle));
                    assert_eq!(CStr::from_ptr(first).to_bytes(), expected);
                    assert_eq!(CStr::from_ptr(second).to_bytes(), expected);
                    if !expected.is_empty() {
                        *first = 42;
                    }
                    assert_eq!(CStr::from_ptr(second).to_bytes(), expected);
                } else {
                    assert!(first.is_null() && second.is_null());
                }
                obs_util::ffi::darray::bfree(first.cast());
                obs_util::ffi::darray::bfree(second.cast());
            }
            if !pointer.is_null() {
                assert_ne!(oracle_encoder_error_read(rust.0), pointer);
                assert_ne!(oracle_encoder_error_read(c.0), pointer);
            }
        }
    }
}
#[test]
fn characterization_and_edges() {
    check(&[
        Some(b"boom".to_vec()),
        None,
        Some(vec![]),
        Some(vec![0xff, 0x80]),
        Some(b"a\0b".to_vec()),
        Some(b"replacement".to_vec()),
        None,
        None,
    ]);
}
#[test]
fn exact_null_diagnostic() {
    if !super::isolated(concat!(module_path!(), "::exact_null_diagnostic")) {
        return;
    }
    // SAFETY: isolated child exclusively owns the process-wide log handler.
    unsafe {
        assert!(oracle_encoder_error_null_log(
            oracle_obs_encoder_set_last_error
        ));
        assert!(oracle_encoder_error_null_log(obs_encoder_set_last_error));
        // NULL output must be validated before even reading a message pointer.
        obs_encoder_set_last_error(core::ptr::null_mut(), core::ptr::dangling());
    }
}
proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn random_replacement_sequences(messages in prop::collection::vec(prop::option::of(prop::collection::vec(any::<u8>(), 0..=256)), 0..=24)) {
        check(&messages);
    }
}

#[test]
fn libobs_allocator_ownership() {
    if !super::isolated(concat!(module_path!(), "::libobs_allocator_ownership")) {
        return;
    }
    // SAFETY: isolated child owns allocator counters and the fixture exclusively.
    unsafe {
        let baseline = obs_util::ffi::bmem::bnum_allocs();
        let output = Fixture::new();
        for message in [c"boom", c"", c"replacement"] {
            obs_encoder_set_last_error(output.0, message.as_ptr());
            assert_eq!(obs_util::ffi::bmem::bnum_allocs(), baseline + 1);
        }
        obs_encoder_set_last_error(output.0, core::ptr::null());
        assert_eq!(obs_util::ffi::bmem::bnum_allocs(), baseline);
        obs_encoder_set_last_error(output.0, c"freed by output destruction".as_ptr());
        drop(output);
        assert_eq!(obs_util::ffi::bmem::bnum_allocs(), baseline);
    }
}

unsafe extern "C" {
    fn oracle_encoder_error_locking(
        set: unsafe extern "C" fn(*mut c_void, *const c_char),
        dup: unsafe extern "C" fn(*mut c_void) -> *mut c_char,
    );
}
#[test]
fn test_encoder_error_ownership_locking() {
    // SAFETY: helper owns the real encoder, joins every thread, frees all errors.
    unsafe {
        oracle_encoder_error_locking(
            oracle_obs_encoder_set_last_error,
            oracle_obs_encoder_dup_last_error,
        );
        oracle_encoder_error_locking(obs_encoder_set_last_error, obs_encoder_dup_last_error);
    }
}

#[test]
fn owned_duplicates_survive_replacement() {
    if !super::isolated(concat!(
        module_path!(),
        "::owned_duplicates_survive_replacement"
    )) {
        return;
    }
    // SAFETY: isolated process owns allocation counters and live encoder fixtures;
    // all returned bmem allocations are released exactly once.
    unsafe {
        for (set, dup) in [
            (
                oracle_obs_encoder_set_last_error
                    as unsafe extern "C" fn(*mut c_void, *const c_char),
                oracle_obs_encoder_dup_last_error
                    as unsafe extern "C" fn(*mut c_void) -> *mut c_char,
            ),
            (obs_encoder_set_last_error, obs_encoder_dup_last_error),
        ] {
            let baseline = obs_util::ffi::bmem::bnum_allocs();
            let encoder = Fixture::new();
            set(encoder.0, c"original".as_ptr());
            let first = dup(encoder.0);
            let second = dup(encoder.0);
            assert_eq!(obs_util::ffi::bmem::bnum_allocs(), baseline + 3);
            *first = b'X' as c_char;
            set(encoder.0, c"replacement".as_ptr());
            assert_eq!(obs_util::ffi::bmem::bnum_allocs(), baseline + 3);
            assert_eq!(CStr::from_ptr(first).to_bytes(), b"Xriginal");
            assert_eq!(CStr::from_ptr(second).to_bytes(), b"original");
            assert_eq!(encoder.message(), Some(b"replacement".to_vec()));
            obs_util::ffi::darray::bfree(first.cast());
            obs_util::ffi::darray::bfree(second.cast());
            drop(encoder);
            assert_eq!(obs_util::ffi::bmem::bnum_allocs(), baseline);
        }
    }
}
