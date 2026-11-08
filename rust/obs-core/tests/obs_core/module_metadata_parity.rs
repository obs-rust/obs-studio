//! Exact C parity, including pointer identity and callback counts. No intended
//! differences. Only null or live opaque handles are valid; private layouts stay C.
use obs_core::ffi::module_metadata::*;
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use std::ffi::{c_char, c_void};

unsafe extern "C" {
    fn oracle_module_metadata_new(
        flags: u32,
        text: *mut c_char,
        empty: *mut c_char,
        other: *mut c_char,
    ) -> *mut c_void;
    fn oracle_module_metadata_free(module: *mut c_void);
    fn oracle_module_metadata_reset_calls() -> u32;
    fn oracle_module_metadata_field_addresses(module: *mut c_void) -> bool;
    fn oracle_obs_get_module_file_name(module: *mut c_void) -> *const c_char;
    fn oracle_obs_get_module_name(module: *mut c_void) -> *const c_char;
    fn oracle_obs_get_module_author(module: *mut c_void) -> *const c_char;
    fn oracle_obs_get_module_description(module: *mut c_void) -> *const c_char;
    fn oracle_obs_get_module_binary_path(module: *mut c_void) -> *const c_char;
    fn oracle_obs_get_module_data_path(module: *mut c_void) -> *const c_char;
    fn oracle_obs_get_module_id(module: *mut c_void) -> *const c_char;
    fn oracle_obs_get_module_version(module: *mut c_void) -> *const c_char;
    fn oracle_obs_get_module_lib(module: *mut c_void) -> *mut c_void;
}

type Getter<T> = unsafe extern "C" fn(*mut c_void) -> T;

// The caller holds a live C fixture (or null) and its borrowed buffers.
unsafe fn check<T: std::fmt::Debug + PartialEq>(
    module: *mut c_void,
    label: &str,
    flags: u16,
    rust: Getter<T>,
    oracle: Getter<T>,
) {
    // SAFETY: both getter functions receive the same live opaque fixture;
    // only this single parity test uses the C callback counter.
    unsafe {
        oracle_module_metadata_reset_calls();
        let expected = oracle(module);
        let expected_calls = oracle_module_metadata_reset_calls();
        assert_eq!(rust(module), expected, "{label} flags={flags}");
        assert_eq!(
            oracle_module_metadata_reset_calls(),
            expected_calls,
            "{label} calls flags={flags}"
        );
    }
}

fn compare(flags: u16, bytes: &[u8]) {
    let mut text = bytes.to_vec();
    text.push(0);
    let mut empty = [0u8];
    let mut other = [b'o', 0];
    // SAFETY: C constructs a real private module, borrowing these live buffers.
    // The fixture is freed after all getters; none owns or changes these bytes.
    unsafe {
        let fixture = oracle_module_metadata_new(
            u32::from(flags),
            text.as_mut_ptr().cast(),
            empty.as_mut_ptr().cast(),
            other.as_mut_ptr().cast(),
        );
        assert!(!fixture.is_null());
        assert!(oracle_module_metadata_field_addresses(fixture));
        let module = if flags & 1 != 0 {
            fixture
        } else {
            std::ptr::null_mut()
        };
        check(
            module,
            "file_name",
            flags,
            obs_get_module_file_name,
            oracle_obs_get_module_file_name,
        );
        check(
            module,
            "name",
            flags,
            obs_get_module_name,
            oracle_obs_get_module_name,
        );
        check(
            module,
            "author",
            flags,
            obs_get_module_author,
            oracle_obs_get_module_author,
        );
        check(
            module,
            "description",
            flags,
            obs_get_module_description,
            oracle_obs_get_module_description,
        );
        check(
            module,
            "binary_path",
            flags,
            obs_get_module_binary_path,
            oracle_obs_get_module_binary_path,
        );
        check(
            module,
            "data_path",
            flags,
            obs_get_module_data_path,
            oracle_obs_get_module_data_path,
        );
        check(
            module,
            "id",
            flags,
            obs_get_module_id,
            oracle_obs_get_module_id,
        );
        check(
            module,
            "version",
            flags,
            obs_get_module_version,
            oracle_obs_get_module_version,
        );
        check(
            module,
            "lib",
            flags,
            obs_get_module_lib,
            oracle_obs_get_module_lib,
        );
        oracle_module_metadata_free(fixture);
    }
    assert_eq!(&text[..bytes.len()], bytes);
    assert_eq!(empty, [0]);
    assert_eq!(other, [b'o', 0]);
}

#[test]
fn original_c_parity() {
    // Exhaust every branch/callback combination on every parity level.
    for flags in 0..1024 {
        compare(flags, &[b'x', 0xff]);
    }
    let mut runner = TestRunner::new(Config {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..Config::default()
    });
    runner
        .run(
            &(any::<u16>(), proptest::collection::vec(any::<u8>(), 0..64)),
            |(flags, bytes)| {
                compare(flags, &bytes);
                Ok(())
            },
        )
        .expect("original C module metadata parity");
}
