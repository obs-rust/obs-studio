//! Real-header layout and Tier 3 parity for #4122; no input exclusions.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_source::ffi::source_type_data::{Fields, obs_source_get_type_data as get};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_source_type_data_new(present: bool, same: bool, flags: u32) -> *mut c_void;
    fn oracle_source_type_data_tokens(source: *mut c_void, tokens: *mut *mut c_void);
    fn oracle_source_type_data_free(source: *mut c_void);
    fn oracle_source_type_data_unchanged(source: *const c_void) -> bool;
    fn oracle_source_type_data_addresses(source: *const c_void) -> bool;
    fn oracle_source_type_data_layout(layout: *mut usize);
    fn oracle_obs_source_get_type_data(source: *mut c_void) -> *mut c_void;
    fn oracle_source_type_data_logs(getter: Getter, get_log: GetLog, set_log: SetLog) -> bool;
}

struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: owns the C allocation; type-data tokens are owned by this fixture.
        unsafe { oracle_source_type_data_free(self.0) }
    }
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 5];
    // SAFETY: the oracle writes exactly five size/alignment/offset values.
    unsafe { oracle_source_type_data_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, data),
            size_of::<*mut c_void>(),
            align_of::<*mut c_void>()
        ]
    );
}

fn check(present: bool, same: bool, flags: u32) {
    // SAFETY: the oracle owns its long-double-aligned type-data tokens and fixture.
    let fixture = Fixture(unsafe { oracle_source_type_data_new(present, same, flags) });
    assert!(!fixture.0.is_null());
    let mut tokens = [core::ptr::null_mut(); 2];
    // SAFETY: this live fixture owns both tokens; the oracle writes exactly two addresses.
    unsafe { oracle_source_type_data_tokens(fixture.0, tokens.as_mut_ptr()) };
    let expected = if present {
        tokens[usize::from(!same)]
    } else {
        core::ptr::null_mut()
    };
    // SAFETY: the real-layout fixture is live; only the diagnostic test supplies
    // NULL to public getters, never to a member-address accessor.
    unsafe {
        assert!(oracle_source_type_data_addresses(fixture.0));
        for _ in 0..2 {
            assert_eq!(get(fixture.0), oracle_obs_source_get_type_data(fixture.0));
            assert_eq!(get(fixture.0), expected);
            assert!(oracle_source_type_data_unchanged(fixture.0));
        }
    }
}

#[test]
fn characterized_edges() {
    for present in [false, true] {
        for same in [false, true] {
            for flags in [0, u32::MAX] {
                check(present, same, flags);
            }
        }
    }
}

#[test]
fn null_diagnostics_match_c() {
    if !crate::output::isolated(concat!(module_path!(), "::null_diagnostics_match_c")) {
        return;
    }
    // SAFETY: the isolated child owns the log handler; callbacks use the base
    // shim's opaque handler-pointer ABI and the probe restores the handler.
    unsafe {
        assert!(oracle_source_type_data_logs(
            get,
            base_get_log_handler,
            base_set_log_handler
        ));
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn arbitrary_type_data_match_c(present in any::<bool>(), same in any::<bool>(), flags in any::<u32>()) {
        check(present, same, flags);
    }
}
