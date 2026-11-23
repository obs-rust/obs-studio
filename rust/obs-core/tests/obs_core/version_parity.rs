//! Tier 3: the original `obs-version.c.h` against the Rust, with the numbers
//! of the real `obs-config.h` and the production `OBS_VERSION` adapter.
use obs_core::ffi::version::{obs_get_version, obs_get_version_string};
use obs_core::version::{
    LIBOBS_API_MAJOR_VER, LIBOBS_API_MINOR_VER, LIBOBS_API_PATCH_VER, make_semantic_version,
};
use proptest::prelude::*;
use std::ffi::{CStr, c_char};
use std::sync::Mutex;

static LOCK: Mutex<()> = Mutex::new(());

unsafe extern "C" {
    fn oracle_obs_get_version() -> u32;
    fn oracle_obs_get_version_string() -> *const c_char;
    fn oracle_version_set_string(version: *const c_char);
    fn oracle_version_parts(parts: *mut u32);
    fn oracle_make_semantic_version(major: u32, minor: u32, patch: u32) -> u32;
    fn oracle_version_layout(layout: *mut usize);
}

#[test]
fn real_header_numbers_and_layout() {
    let mut parts = [0; 4];
    let mut layout = [0; 3];
    // SAFETY: the oracle writes exactly four u32 and three size_t entries.
    unsafe {
        oracle_version_parts(parts.as_mut_ptr());
        oracle_version_layout(layout.as_mut_ptr());
        assert_eq!(obs_get_version(), oracle_obs_get_version());
    }
    assert_eq!(
        parts,
        [
            LIBOBS_API_MAJOR_VER,
            LIBOBS_API_MINOR_VER,
            LIBOBS_API_PATCH_VER,
            obs_get_version()
        ]
    );
    assert_eq!(
        layout,
        [
            size_of::<u32>(),
            size_of::<*const c_char>(),
            align_of::<*const c_char>()
        ]
    );
}

fn compare_string(version: *const c_char) {
    // SAFETY: the test owns the oracle global under LOCK. Neither side
    // dereferences the pointer; both return it as stored.
    unsafe {
        oracle_version_set_string(version);
        let rust = obs_get_version_string();
        assert_eq!(rust, oracle_obs_get_version_string());
        assert_eq!(rust, version);
        assert_eq!(obs_get_version_string(), rust);
    }
}

#[test]
fn string_is_the_global_pointer_itself() {
    let _guard = LOCK.lock().unwrap();
    let first = c"33.0.0-test";
    let second = c"";
    compare_string(first.as_ptr());
    // SAFETY: both sides return the pointer just stored, which is `first`.
    assert_eq!(unsafe { CStr::from_ptr(obs_get_version_string()) }, first);
    compare_string(second.as_ptr());
    compare_string(std::ptr::null());
    compare_string(first.as_ptr());
}

#[test]
fn packing_edges() {
    for (major, minor, patch) in [
        (0, 0, 0),
        (33, 0, 0),
        (30, 2, 3),
        (255, 255, 0xffff),
        (256, 0, 0),
        (0, 0x1_0000, 0),
        (0, 0, 0x1_0000),
        (u32::MAX, u32::MAX, u32::MAX),
    ] {
        // SAFETY: a pure function of its arguments.
        let c = unsafe { oracle_make_semantic_version(major, minor, patch) };
        assert_eq!(make_semantic_version(major, minor, patch), c);
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn packing_matches_c(major in any::<u32>(), minor in any::<u32>(), patch in any::<u32>()) {
        // SAFETY: a pure function of its arguments.
        let c = unsafe { oracle_make_semantic_version(major, minor, patch) };
        prop_assert_eq!(make_semantic_version(major, minor, patch), c);
    }

    #[test]
    fn string_matches_c(address in any::<usize>()) {
        let _guard = LOCK.lock().unwrap();
        compare_string(std::ptr::without_provenance(address));
    }
}
