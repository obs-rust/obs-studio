//! Tier 3 runs the original C getter and Rust shim on real private-layout sources.
use core::ffi::c_void;
use obs_source::ffi::source_filter_index::obs_source_filter_get_index as get_index;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_filter_index_new(ids: *const u8, count: usize) -> *mut c_void;
    fn oracle_filter_index_token(source: *mut c_void, id: u32) -> *mut c_void;
    fn oracle_filter_index_unchanged(source: *mut c_void) -> bool;
    fn oracle_filter_index_free(source: *mut c_void) -> bool;
    fn oracle_obs_source_filter_get_index(source: *mut c_void, filter: *mut c_void) -> i32;
    fn oracle_filter_index_logs(getter: Getter, get: GetLog, set: SetLog) -> bool;
}

struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: fixture exclusively owns the source and its initialized mutex.
        assert!(unsafe { oracle_filter_index_free(self.0) });
    }
}

fn check(ids: &[u8], target: u32) {
    // SAFETY: bounded input stays readable while the oracle builds an owned fixture.
    let fixture = Fixture(unsafe { oracle_filter_index_new(ids.as_ptr(), ids.len()) });
    assert!(!fixture.0.is_null());
    // SAFETY: all handles refer to live real C objects; each getter releases its lock.
    unsafe {
        let filter = oracle_filter_index_token(fixture.0, target);
        for _ in 0..2 {
            let actual = get_index(fixture.0, filter);
            assert!(oracle_filter_index_unchanged(fixture.0));
            assert_eq!(
                actual,
                oracle_obs_source_filter_get_index(fixture.0, filter)
            );
            assert_eq!(
                actual,
                ids.iter()
                    .position(|&id| u32::from(id) == target)
                    .map_or(-1, |i| i as i32)
            );
            assert!(oracle_filter_index_unchanged(fixture.0));
        }
    }
}

#[test]
fn ordered_empty_absent_duplicate_and_boundaries() {
    for ids in [
        &[][..],
        &[2, 0, 1, 0],
        &[7],
        &[0, 1, 2, 3, 4, 5, 6, 7],
        &[0; 64],
    ] {
        for target in 0..=8 {
            check(ids, target);
        }
    }
}

#[test]
fn null_diagnostics_match_c() {
    if !crate::output::isolated(concat!(module_path!(), "::null_diagnostics_match_c")) {
        return;
    }
    // SAFETY: isolated child owns the process log handler; the oracle restores it.
    assert!(unsafe {
        oracle_filter_index_logs(get_index, base_get_log_handler, base_set_log_handler)
    });
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn ordered_arrays_match_c(ids in prop::collection::vec(0u8..8, 0..=64), target in 0u32..=8) {
        check(&ids, target);
    }
}
