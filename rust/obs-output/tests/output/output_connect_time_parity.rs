//! Exact differential and real-header layout coverage for #4128; no exclusions.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::output_connect_time::{Fields, obs_output_get_connect_time_ms};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_output_connect_time_layout(layout: *mut usize);
    fn oracle_output_connect_time_new(callback: bool, result: i32, null_data: bool) -> *mut c_void;
    fn oracle_output_connect_time_unchanged(output: *mut c_void, calls: u32) -> bool;
    fn oracle_output_connect_time_free(output: *mut c_void);
    fn oracle_obs_output_get_connect_time_ms(output: *mut c_void) -> i32;
    fn oracle_output_connect_time_null_log(rust: bool) -> bool;
    fn oracle_output_connect_time_valid_silent(rust: bool, output: *mut c_void) -> bool;
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 4];
    // SAFETY: oracle writes four size_t values.
    unsafe { oracle_output_connect_time_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, data),
            offset_of!(Fields, callback)
        ]
    );
}

struct Output(*mut c_void);
impl Drop for Output {
    fn drop(&mut self) {
        // SAFETY: oracle allocation owned exclusively by this wrapper.
        unsafe { oracle_output_connect_time_free(self.0) };
    }
}
fn check(callback: bool, result: i32, null_data: bool) {
    // SAFETY: oracle owns a real output with a synchronous callback.
    let output = Output(unsafe { oracle_output_connect_time_new(callback, result, null_data) });
    assert!(!output.0.is_null());
    // SAFETY: live fixture and plugin data remain valid through both calls.
    unsafe {
        assert_eq!(
            obs_output_get_connect_time_ms(output.0),
            if !callback {
                -1
            } else if null_data {
                -73
            } else {
                result
            }
        );
        assert!(oracle_output_connect_time_unchanged(
            output.0,
            u32::from(callback && !null_data)
        ));
        assert_eq!(
            oracle_obs_output_get_connect_time_ms(output.0),
            if !callback {
                -1
            } else if null_data {
                -73
            } else {
                result
            }
        );
        assert!(oracle_output_connect_time_unchanged(
            output.0,
            2 * u32::from(callback && !null_data)
        ));
    }
}
#[test]
fn characterized_signed_results() {
    for callback in [false, true] {
        for result in [i32::MIN, -97, -1, 0, 29, i32::MAX] {
            for null_data in [false, true] {
                check(callback, result, null_data);
            }
        }
    }
}

#[test]
fn null_default_and_log() {
    if !super::isolated(concat!(module_path!(), "::null_default_and_log")) {
        return;
    }
    // SAFETY: each probe saves/restores the global logger in this isolated child.
    unsafe {
        assert!(oracle_output_connect_time_null_log(false));
        assert!(oracle_output_connect_time_null_log(true));
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn random_results_match_c(callback in any::<bool>(), result in any::<i32>(), null_data in any::<bool>()) {
        check(callback, result, null_data);
    }
}

#[test]
fn valid_outputs_emit_no_diagnostic() {
    if !super::isolated(concat!(
        module_path!(),
        "::valid_outputs_emit_no_diagnostic"
    )) {
        return;
    }
    for rust in [false, true] {
        for callback in [false, true] {
            for null_data in [false, true] {
                // SAFETY: oracle creates a real output with synchronous plugin data.
                let output =
                    Output(unsafe { oracle_output_connect_time_new(callback, 23, null_data) });
                assert!(!output.0.is_null());
                // SAFETY: live fixture; logger capture runs in this isolated child.
                assert!(unsafe { oracle_output_connect_time_valid_silent(rust, output.0) });
            }
        }
    }
}
