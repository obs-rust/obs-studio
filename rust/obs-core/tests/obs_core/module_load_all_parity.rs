//! Tier 3: the original C no-ops and the Rust exports over the same caller
//! failure info. No intentional differences: neither reads or writes it.
use obs_core::ffi::module_load_all::{obs_load_all_modules, obs_load_all_modules2};
use proptest::prelude::*;
use std::ffi::{c_char, c_void};

unsafe extern "C" {
    fn oracle_obs_load_all_modules();
    fn oracle_obs_load_all_modules2(mfi: *mut c_void);
}

/// `struct obs_module_failure_info` (obs-module.h): `char **failed_modules;
/// size_t count;`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
struct FailureInfo {
    failed_modules: *mut *mut c_char,
    count: usize,
}

/// Run one implementation three times over caller storage holding `count`
/// and return the count and the pointed-to strings afterwards.
fn observe(rust: bool, count: usize, null: bool) -> (usize, Vec<u8>, Vec<u8>) {
    let mut first = *b"failed-module-sentinel\0";
    let mut second = *b"another-sentinel\0";
    let mut failed = [
        first.as_mut_ptr().cast::<c_char>(),
        second.as_mut_ptr().cast(),
        std::ptr::null_mut(),
    ];
    let failed_before = failed;
    // Deliberately not the array length: even the count is caller-owned.
    let mut info = FailureInfo {
        failed_modules: failed.as_mut_ptr(),
        count,
    };
    let before = info;
    let mfi = if null {
        std::ptr::null_mut()
    } else {
        (&raw mut info).cast::<c_void>()
    };
    for _ in 0..3 {
        // SAFETY: both implementations accept NULL or live failure info, and
        // neither dereferences it.
        unsafe {
            if rust {
                obs_load_all_modules();
                obs_load_all_modules2(mfi);
            } else {
                oracle_obs_load_all_modules();
                oracle_obs_load_all_modules2(mfi);
            }
        }
    }
    assert_eq!(info, before);
    assert_eq!(failed, failed_before);
    (info.count, first.to_vec(), second.to_vec())
}

#[test]
fn characterization_case_matches_c() {
    let rust = observe(true, usize::MAX - 17, false);
    assert_eq!(rust, observe(false, usize::MAX - 17, false));
    assert_eq!(rust.0, usize::MAX - 17);
    assert_eq!(&rust.1[..], b"failed-module-sentinel\0");
    assert_eq!(&rust.2[..], b"another-sentinel\0");
    assert_eq!(observe(true, 0, true), observe(false, 0, true));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn rust_matches_c_over_caller_storage(count in any::<usize>(), null in any::<bool>()) {
        prop_assert_eq!(observe(true, count, null), observe(false, count, null));
    }
}

#[test]
fn exported_rust_symbols_keep_c_signatures() {
    let _: extern "C" fn() = obs_load_all_modules;
    let _: extern "C" fn(*mut c_void) = obs_load_all_modules2;
}
