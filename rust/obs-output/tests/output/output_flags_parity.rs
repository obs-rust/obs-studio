//! Real-header layout, full-width flags, immutable state and exact diagnostics.
//! No intentional differences or input exclusions.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::output_flags::{Fields, obs_output_get_flags as flags};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*const c_void) -> u32;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_output_flags_new(value: u32, mask: usize) -> *mut c_void;
    fn oracle_output_flags_free(output: *mut c_void);
    fn oracle_output_flags_unchanged(output: *const c_void) -> bool;
    fn oracle_output_flags_addresses(output: *const c_void) -> bool;
    fn oracle_output_flags_layout(layout: *mut usize);
    fn oracle_obs_output_get_flags(output: *const c_void) -> u32;
    fn oracle_output_flags_logs(getter: Getter, get: GetLog, set: SetLog) -> bool;
}

struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: the fixture owns exactly this C allocation.
        unsafe { oracle_output_flags_free(self.0) }
    }
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 5];
    // SAFETY: the oracle writes five layout entries.
    unsafe { oracle_output_flags_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, flags),
            size_of::<u32>(),
            align_of::<u32>()
        ]
    );
}

fn check(value: u32, mask: usize) {
    // SAFETY: fixture constructor takes only plain values.
    let fixture = Fixture(unsafe { oracle_output_flags_new(value, mask) });
    assert!(!fixture.0.is_null());
    // SAFETY: real-layout fixture is live; the field adapter never receives NULL.
    unsafe {
        assert!(oracle_output_flags_addresses(fixture.0));
        for _ in 0..2 {
            assert_eq!(flags(fixture.0), oracle_obs_output_get_flags(fixture.0));
            assert_eq!(flags(fixture.0), value);
            assert!(oracle_output_flags_unchanged(fixture.0));
        }
    }
}

#[test]
fn characterized_edges() {
    for value in [0, 1, 3, 7, 0x8000_0000, 0xaaaa_aaaa, u32::MAX]
        .into_iter()
        .chain((0..32).map(|bit| 1u32 << bit))
    {
        for mask in [0, 65, usize::MAX] {
            check(value, mask);
        }
    }
}

#[test]
fn null_diagnostics_match_c() {
    if !super::isolated(concat!(module_path!(), "::null_diagnostics_match_c")) {
        return;
    }
    // SAFETY: isolated child exclusively owns the log handler. Probe restores
    // it and invokes production symbols through the supplied callbacks only.
    unsafe {
        assert!(oracle_output_flags_logs(
            flags,
            base_get_log_handler,
            base_set_log_handler
        ));
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn arbitrary_flags_match_c(value in any::<u32>(), mask in any::<usize>()) { check(value, mask); }
}
