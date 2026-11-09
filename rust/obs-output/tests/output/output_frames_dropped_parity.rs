//! Exact real-header differential coverage for #4118. No exclusions.
use core::ffi::{c_int, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::output_frames_dropped::{Fields, obs_output_get_frames_dropped};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_output_frames_dropped_layout(layout: *mut usize);
    fn oracle_output_frames_dropped_new(
        result: c_int,
        callback: bool,
        null_data: bool,
    ) -> *mut c_void;
    fn oracle_output_frames_dropped_state(output: *mut c_void, calls: u32) -> bool;
    fn oracle_output_frames_dropped_reset(output: *mut c_void);
    fn oracle_output_frames_dropped_free(output: *mut c_void);
    fn oracle_obs_output_get_frames_dropped(output: *const c_void) -> c_int;
    fn oracle_output_frames_dropped_null_log(rust: bool) -> bool;
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 6];
    // SAFETY: oracle writes exactly six size_t values.
    unsafe { oracle_output_frames_dropped_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, callback),
            offset_of!(Fields, data),
            size_of::<c_int>(),
            align_of::<c_int>()
        ]
    );
}

struct Output(*mut c_void);
impl Drop for Output {
    fn drop(&mut self) {
        // SAFETY: exclusively owned fixed-size oracle allocation.
        unsafe { oracle_output_frames_dropped_free(self.0) };
    }
}
fn check(result: i32, callback: bool, null_data: bool) {
    // SAFETY: constructor initializes a real output and context without external resources.
    let output = Output(unsafe { oracle_output_frames_dropped_new(result, callback, null_data) });
    assert!(!output.0.is_null());
    let calls = u32::from(callback && !null_data);
    // SAFETY: getters borrow the live fixture and valid plugin context sequentially.
    unsafe {
        let expected = oracle_obs_output_get_frames_dropped(output.0);
        assert!(oracle_output_frames_dropped_state(output.0, calls));
        oracle_output_frames_dropped_reset(output.0);
        assert_eq!(obs_output_get_frames_dropped(output.0), expected);
        assert!(oracle_output_frames_dropped_state(output.0, calls));
    }
}

#[test]
fn characterized_defaults_and_signed_edges() {
    for result in [i32::MIN, -1, 0, 1, i32::MAX] {
        for callback in [false, true] {
            for null_data in [false, true] {
                check(result, callback, null_data);
            }
        }
    }
}

#[test]
fn null_default_and_exact_log() {
    if !super::isolated(concat!(module_path!(), "::null_default_and_exact_log")) {
        return;
    }
    // SAFETY: each probe saves/restores the logger in this isolated child.
    unsafe {
        assert!(oracle_output_frames_dropped_null_log(false));
        assert!(oracle_output_frames_dropped_null_log(true));
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn random_results_match_c(result in any::<i32>(), callback in any::<bool>(), null_data in any::<bool>()) {
        check(result, callback, null_data);
    }
}
