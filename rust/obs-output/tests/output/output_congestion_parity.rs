//! Exact parity against the original query and C clamp; no exclusions.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::output_congestion::{Fields, obs_output_get_congestion};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_output_congestion_layout(layout: *mut usize);
    fn oracle_output_congestion_new(bits: u32, callback: bool) -> *mut c_void;
    fn oracle_output_congestion_unchanged(output: *mut c_void, calls: u32) -> bool;
    fn oracle_output_congestion_free(output: *mut c_void);
    fn oracle_obs_output_get_congestion(output: *mut c_void) -> f32;
    fn oracle_output_congestion_null_log(rust: bool) -> bool;
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 4];
    // SAFETY: oracle writes exactly four size_t values.
    unsafe { oracle_output_congestion_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, callback),
            offset_of!(Fields, data)
        ]
    );
}

struct Output(*mut c_void);
impl Drop for Output {
    fn drop(&mut self) {
        // SAFETY: exclusively owned, resource-free C fixture allocation.
        unsafe { oracle_output_congestion_free(self.0) };
    }
}
fn check(bits: u32, callback: bool) {
    // SAFETY: constructs the real private layout with an optional test callback.
    let output = Output(unsafe { oracle_output_congestion_new(bits, callback) });
    assert!(!output.0.is_null());
    // SAFETY: both getters borrow the live fixture; callback data points to it.
    unsafe {
        let actual = obs_output_get_congestion(output.0);
        assert!(oracle_output_congestion_unchanged(
            output.0,
            u32::from(callback)
        ));
        let expected = oracle_obs_output_get_congestion(output.0);
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "bits={bits:#010x}, callback={callback}"
        );
        assert!(oracle_output_congestion_unchanged(
            output.0,
            2 * u32::from(callback)
        ));
    }
}

#[test]
fn callback_edges_and_absent_callback_match_c() {
    for bits in [
        0,
        1,
        0x8000_0000,
        0x8000_0001,
        0xbf80_0000,
        0x3e80_0000,
        0x3f00_0000,
        0x3f7f_ffff,
        0x3f80_0000,
        0x3f80_0001,
        0x4000_0000,
        0x7f7f_ffff,
        0xff7f_ffff,
        0x7f80_0000,
        0xff80_0000,
        0x7fc0_0000,
        0xffc0_0000,
        0x7fc0_0001,
        0x7fff_ffff,
        0xffff_ffff,
    ] {
        check(bits, true);
        check(bits, false);
    }
}

#[test]
fn null_default_and_diagnostic_match_c() {
    if !super::isolated(concat!(
        module_path!(),
        "::null_default_and_diagnostic_match_c"
    )) {
        return;
    }
    // SAFETY: probes save/restore the global logger in an isolated process.
    unsafe {
        assert!(oracle_output_congestion_null_log(false));
        assert!(oracle_output_congestion_null_log(true));
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn random_float_bits_match_c(bits in any::<u32>(), callback in any::<bool>()) {
        check(bits, callback);
    }
}
