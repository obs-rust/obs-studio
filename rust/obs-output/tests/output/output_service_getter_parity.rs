//! Real-header layout and Tier 3 parity for #4039; no input exclusions.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::output_service_getter::{Fields, obs_output_get_service as service};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*const c_void) -> *mut c_void;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_output_service_new(which: u32, flags: u32) -> *mut c_void;
    fn oracle_output_service_expected(output: *const c_void) -> *mut c_void;
    fn oracle_output_service_free(output: *mut c_void);
    fn oracle_output_service_unchanged(output: *const c_void) -> bool;
    fn oracle_output_service_addresses(output: *const c_void) -> bool;
    fn oracle_output_service_layout(layout: *mut usize);
    fn oracle_obs_output_get_service(output: *const c_void) -> *mut c_void;
    fn oracle_output_service_logs(getter: Getter, get: GetLog, set: SetLog) -> bool;
}

struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: owns the C allocation, including both service tokens.
        unsafe { oracle_output_service_free(self.0) }
    }
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 5];
    // SAFETY: the oracle writes exactly five size/alignment/offset values.
    unsafe { oracle_output_service_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, service),
            size_of::<*mut c_void>(),
            align_of::<*mut c_void>()
        ]
    );
}

fn check(which: u32, flags: u32) {
    // SAFETY: the oracle allocates the real output and service layouts.
    let fixture = Fixture(unsafe { oracle_output_service_new(which, flags) });
    assert!(!fixture.0.is_null());
    // SAFETY: the live fixture owns its service tokens. NULL is never supplied
    // to an accessor that takes a member address.
    unsafe {
        assert!(oracle_output_service_addresses(fixture.0));
        for _ in 0..2 {
            assert_eq!(service(fixture.0), oracle_obs_output_get_service(fixture.0));
            assert_eq!(
                service(fixture.0),
                oracle_output_service_expected(fixture.0)
            );
            assert!(oracle_output_service_unchanged(fixture.0));
        }
    }
}

#[test]
fn characterized_edges() {
    for which in 0..3 {
        for flags in [0, u32::MAX] {
            check(which, flags);
        }
    }
}

#[test]
fn null_diagnostics_match_c() {
    if !super::isolated(concat!(module_path!(), "::null_diagnostics_match_c")) {
        return;
    }
    // SAFETY: this isolated process owns the log handler; the probe restores it.
    unsafe {
        assert!(oracle_output_service_logs(
            service,
            base_get_log_handler,
            base_set_log_handler
        ));
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128), failure_persistence: None, .. ProptestConfig::default()
    })]
    #[test]
    fn borrowed_service_matches_c(which in 0_u32..3, flags in any::<u32>()) { check(which, flags); }
}
