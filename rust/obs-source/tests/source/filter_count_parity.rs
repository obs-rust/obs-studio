//! Real-header Tier 2 layout and Tier 3 parity for #4131. No exclusions.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_source::ffi::source_filter_count::{Fields, obs_source_filter_count as count};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*const c_void) -> usize;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_filter_count_new(count: usize) -> *mut c_void;
    fn oracle_filter_count_free(source: *mut c_void);
    fn oracle_filter_count_unchanged(source: *const c_void) -> bool;
    fn oracle_filter_count_address(source: *const c_void) -> bool;
    fn oracle_filter_count_layout(layout: *mut usize);
    fn oracle_obs_source_filter_count(source: *const c_void) -> usize;
    fn oracle_filter_count_logs(getter: Getter, get: GetLog, set: SetLog) -> bool;
}

struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: owns the oracle allocation; no filter-array allocation exists.
        unsafe { oracle_filter_count_free(self.0) }
    }
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 5];
    // SAFETY: the oracle writes exactly five size/alignment/offset values.
    unsafe { oracle_filter_count_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, count),
            size_of::<usize>(),
            align_of::<usize>()
        ]
    );
}

fn check(value: usize) {
    // SAFETY: the oracle creates a real private-layout source; only count is read.
    let fixture = Fixture(unsafe { oracle_filter_count_new(value) });
    assert!(!fixture.0.is_null());
    // SAFETY: fixture remains live; the getter neither dereferences the fake
    // filter array nor frees any source fields. No NULL reaches the adapter.
    unsafe {
        assert!(oracle_filter_count_address(fixture.0));
        for _ in 0..2 {
            assert_eq!(count(fixture.0), oracle_obs_source_filter_count(fixture.0));
            assert_eq!(count(fixture.0), value);
            assert!(oracle_filter_count_unchanged(fixture.0));
        }
    }
}

#[test]
fn edge_counts() {
    for value in [0, 1, 2, u32::MAX as usize, usize::MAX / 2, usize::MAX] {
        check(value);
    }
}

#[test]
fn null_diagnostics_match_c() {
    if !crate::output::isolated(concat!(module_path!(), "::null_diagnostics_match_c")) {
        return;
    }
    // SAFETY: the child exclusively owns the process log handler; oracle restores it.
    unsafe {
        assert!(oracle_filter_count_logs(
            count,
            base_get_log_handler,
            base_set_log_handler
        ));
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn arbitrary_counts_match_c(value in any::<usize>()) {
        check(value);
    }
}
