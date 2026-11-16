//! Exact setter parity, including untouched timing state at integer boundaries.
//! No intentional differences or excluded defined inputs.
use core::ffi::c_void;
use core::mem::{align_of, size_of};
use core::sync::atomic::AtomicBool;
use obs_source::ffi::source_async_compensation::obs_source_set_async_compensation;
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_source_async_compensation_new(
        initial: bool,
        timestamp: u64,
        offset: i64,
    ) -> *mut c_void;
    fn oracle_source_async_compensation_check(source: *mut c_void, expected: bool) -> bool;
    fn oracle_source_async_compensation_free(source: *mut c_void);
    fn oracle_source_async_compensation_layout(layout: *mut usize);
    fn oracle_obs_source_set_async_compensation(source: *mut c_void, requested: bool);
    fn oracle_source_async_compensation_null_log(
        setter: unsafe extern "C" fn(*mut c_void, bool),
        requested: bool,
    ) -> bool;
}

struct Source(*mut c_void);
impl Drop for Source {
    fn drop(&mut self) {
        // SAFETY: exclusively owned C fixture; setter allocates no resources.
        unsafe { oracle_source_async_compensation_free(self.0) };
    }
}

fn check(initial: bool, timestamp: u64, offset: i64, requests: &[bool]) {
    // SAFETY: allocate complete real-layout C fixtures, not Rust layout mirrors.
    let rust = Source(unsafe { oracle_source_async_compensation_new(initial, timestamp, offset) });
    // SAFETY: same independent allocation, released by Drop.
    let c = Source(unsafe { oracle_source_async_compensation_new(initial, timestamp, offset) });
    assert!(!rust.0.is_null() && !c.0.is_null());
    for &requested in requests {
        // SAFETY: live exclusive fixtures; C probes use atomic flag loads.
        unsafe {
            obs_source_set_async_compensation(rust.0, requested);
            oracle_obs_source_set_async_compensation(c.0, requested);
            assert!(oracle_source_async_compensation_check(rust.0, requested));
            assert!(oracle_source_async_compensation_check(c.0, requested));
        }
    }
}

#[test]
fn timestamp_and_offset_edges_are_untouched() {
    for timestamp in [0, 1, i64::MAX as u64, 1_u64 << 63, u64::MAX - 1, u64::MAX] {
        for offset in [i64::MIN, -1, 0, 1, i64::MAX] {
            for initial in [false, true] {
                check(
                    initial,
                    timestamp,
                    offset,
                    &[true, true, false, false, true],
                );
            }
        }
    }
}

#[test]
fn real_header_atomic_layout() {
    let mut layout = [0; 2];
    // SAFETY: C writes exactly two size_t values.
    unsafe { oracle_source_async_compensation_layout(layout.as_mut_ptr()) };
    assert_eq!(layout, [size_of::<AtomicBool>(), align_of::<AtomicBool>()]);
    assert_eq!(layout, [size_of::<bool>(), align_of::<bool>()]);
}

#[test]
fn null_preserves_debug_diagnostic() {
    if std::env::var_os("OBS_ASYNC_COMPENSATION_CHILD").is_none() {
        let selector = concat!(module_path!(), "::null_preserves_debug_diagnostic")
            .split_once("::")
            .unwrap()
            .1;
        assert!(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", selector, "--nocapture"])
                .env("OBS_ASYNC_COMPENSATION_CHILD", "1")
                .status()
                .unwrap()
                .success()
        );
        return;
    }
    for requested in [false, true] {
        // SAFETY: isolated child restores its global logger; NULL is valid here.
        unsafe {
            assert!(oracle_source_async_compensation_null_log(
                oracle_obs_source_set_async_compensation,
                requested
            ));
            assert!(oracle_source_async_compensation_null_log(
                obs_source_set_async_compensation,
                requested
            ));
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(64),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn sequences_match_c(
        initial in any::<bool>(), timestamp in any::<u64>(), offset in any::<i64>(),
        requests in prop::collection::vec(any::<bool>(), 1..32),
    ) {
        check(initial, timestamp, offset, &requests);
    }
}
