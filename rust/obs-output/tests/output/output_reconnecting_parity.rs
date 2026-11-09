//! Exact differential and real-header layout coverage for #4104; no exclusions.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use core::sync::atomic::AtomicBool;
use obs_output::ffi::output_reconnecting::{Fields, obs_output_reconnecting};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_output_reconnecting_layout(layout: *mut usize);
    fn oracle_output_reconnecting_new(flag: bool, active: bool, paused: bool) -> *mut c_void;
    fn oracle_output_reconnecting_unchanged(output: *mut c_void) -> bool;
    fn oracle_output_reconnecting_free(output: *mut c_void);
    fn oracle_obs_output_reconnecting(output: *const c_void) -> bool;
    fn oracle_output_reconnecting_null_log(rust: bool) -> bool;
}

#[test]
fn real_header_layout_and_atomic_alignment() {
    let mut layout = [0; 5];
    // SAFETY: oracle writes exactly five size_t values.
    unsafe { oracle_output_reconnecting_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, reconnecting),
            size_of::<bool>(),
            align_of::<bool>()
        ]
    );
    assert!(layout[4] >= align_of::<AtomicBool>());
}

struct Output(*mut c_void);
impl Drop for Output {
    fn drop(&mut self) {
        // SAFETY: oracle allocation owned exclusively by this wrapper.
        unsafe { oracle_output_reconnecting_free(self.0) };
    }
}
fn check(flag: bool, active: bool, paused: bool) {
    // SAFETY: constructor initializes the real output without external resources.
    let output = Output(unsafe { oracle_output_reconnecting_new(flag, active, paused) });
    assert!(!output.0.is_null());
    // SAFETY: both getters borrow the live fixture with no concurrent non-atomic access.
    unsafe {
        assert_eq!(
            obs_output_reconnecting(output.0),
            oracle_obs_output_reconnecting(output.0)
        );
        assert!(oracle_output_reconnecting_unchanged(output.0));
    }
}

#[test]
fn characterized_flags_independent_of_other_state() {
    for flag in [false, true] {
        for active in [false, true] {
            for paused in [false, true] {
                check(flag, active, paused);
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
        assert!(oracle_output_reconnecting_null_log(false));
        assert!(oracle_output_reconnecting_null_log(true));
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn random_flags_match_c(flag in any::<bool>(), active in any::<bool>(), paused in any::<bool>()) {
        check(flag, active, paused);
    }
}
