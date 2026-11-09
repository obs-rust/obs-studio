//! Exact differential and layout coverage for #4100; no behavior exclusions.
use core::ffi::{c_int, c_long, c_void};
use core::mem::{align_of, offset_of, size_of};
use core::sync::atomic::{AtomicI32, AtomicI64};
use obs_output::ffi::output_total_frames::{Fields, obs_output_get_total_frames};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_output_total_frames_layout(layout: *mut usize);
    fn oracle_output_total_frames_new(count: i64) -> *mut c_void;
    fn oracle_output_total_frames_unchanged(output: *mut c_void) -> bool;
    fn oracle_output_total_frames_free(output: *mut c_void);
    fn oracle_obs_output_get_total_frames(output: *const c_void) -> c_int;
    fn oracle_output_total_frames_null_log(rust: bool) -> bool;
}

#[test]
fn real_header_layout_and_atomic_alignment() {
    let mut layout = [0; 5];
    // SAFETY: oracle writes exactly five size_t values.
    unsafe { oracle_output_total_frames_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, frames),
            size_of::<c_long>(),
            align_of::<c_long>()
        ]
    );
    let atomic_alignment = if size_of::<c_long>() == 8 {
        align_of::<AtomicI64>()
    } else {
        align_of::<AtomicI32>()
    };
    assert!(align_of::<c_long>() >= atomic_alignment);
}

struct Output(*mut c_void);
impl Drop for Output {
    fn drop(&mut self) {
        // SAFETY: oracle allocation owned exclusively by this wrapper.
        unsafe { oracle_output_total_frames_free(self.0) };
    }
}
fn check(count: i64) {
    // SAFETY: constructor initializes the real output without external resources.
    let output = Output(unsafe { oracle_output_total_frames_new(count) });
    assert!(!output.0.is_null());
    // SAFETY: both getters borrow the live fixture, with no concurrent non-atomic access.
    unsafe {
        assert_eq!(
            obs_output_get_total_frames(output.0),
            oracle_obs_output_get_total_frames(output.0)
        );
        assert!(oracle_output_total_frames_unchanged(output.0));
    }
}

#[test]
fn characterized_edges() {
    for count in [
        0,
        1,
        -1,
        i32::MIN as i64,
        i32::MAX as i64,
        i32::MAX as i64 + 1,
        u32::MAX as i64,
        1i64 << 32,
        i64::MIN,
        i64::MAX,
    ] {
        check(count);
    }
}

#[test]
fn null_default_and_log() {
    if !super::isolated(concat!(module_path!(), "::null_default_and_log")) {
        return;
    }
    // SAFETY: each probe saves/restores the global logger in this isolated child.
    unsafe {
        assert!(oracle_output_total_frames_null_log(false));
        assert!(oracle_output_total_frames_null_log(true));
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn random_counts_match_c(count in any::<i64>()) {
        check(count);
    }
}
