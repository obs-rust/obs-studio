//! Tier 2 layout and Tier 3 borrowed-pointer, state and diagnostic parity.
//! No input exclusions or intentional differences.
use core::ffi::{c_char, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::output_identity::{
    Fields, obs_output_get_id as id, obs_output_get_type_data as data,
};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type IdGetter = unsafe extern "C" fn(*const c_void) -> *const c_char;
type DataGetter = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_output_identity_new(id: *const c_char, data: *mut c_void, flags: u32) -> *mut c_void;
    fn oracle_output_identity_free(output: *mut c_void);
    fn oracle_output_identity_unchanged(output: *const c_void) -> bool;
    fn oracle_output_identity_addresses(output: *const c_void) -> bool;
    fn oracle_output_identity_layout(layout: *mut usize);
    fn oracle_obs_output_get_id(output: *const c_void) -> *const c_char;
    fn oracle_obs_output_get_type_data(output: *mut c_void) -> *mut c_void;
    fn oracle_output_identity_logs(
        id: IdGetter,
        data: DataGetter,
        get: GetLog,
        set: SetLog,
    ) -> bool;
}

struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: owns the C allocation; borrowed metadata is still live here.
        unsafe { oracle_output_identity_free(self.0) }
    }
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 4];
    // SAFETY: the oracle writes exactly four size/offset values.
    unsafe { oracle_output_identity_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, id),
            offset_of!(Fields, type_data)
        ]
    );
}

fn check(v: Option<&[u8]>, present_data: bool, flags: u32) {
    let vp = v.map_or(core::ptr::null(), |s| s.as_ptr().cast());
    let mut marker = 17;
    let ap = if present_data {
        core::ptr::from_mut(&mut marker).cast()
    } else {
        core::ptr::null_mut()
    };
    // SAFETY: the C fixture only borrows these byte buffers and never decodes them.
    let fixture = Fixture(unsafe { oracle_output_identity_new(vp, ap, flags) });
    assert!(!fixture.0.is_null());
    // SAFETY: the real-layout fixture stays live; only public getters receive
    // null in the separate diagnostic test, never a member-address accessor.
    unsafe {
        assert!(oracle_output_identity_addresses(fixture.0));
        for _ in 0..2 {
            assert_eq!(id(fixture.0), oracle_obs_output_get_id(fixture.0));
            assert_eq!(data(fixture.0), oracle_obs_output_get_type_data(fixture.0));
            assert_eq!(id(fixture.0), vp);
            assert_eq!(data(fixture.0), ap);
            assert!(oracle_output_identity_unchanged(fixture.0));
        }
    }
}

#[test]
fn characterized_edges() {
    let values: [Option<&[u8]>; 4] = [None, Some(b"\0"), Some(b"h264;\xff\0"), Some(b"aac;opus\0")];
    for (i, value) in values.iter().enumerate() {
        for present_data in [false, true] {
            check(*value, present_data, i as u32);
        }
    }
    check(Some(b"same\0"), true, u32::MAX);
}

#[test]
fn null_diagnostics_match_c() {
    if !super::isolated(concat!(module_path!(), "::null_diagnostics_match_c")) {
        return;
    }
    // SAFETY: the isolated child exclusively owns the log handler; callbacks
    // use the base shim's opaque handler-pointer ABI, and the probe restores it.
    unsafe {
        assert!(oracle_output_identity_logs(
            id,
            data,
            base_get_log_handler,
            base_set_log_handler
        ));
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn arbitrary_metadata_matches_c(mut v in prop::collection::vec(any::<u8>(), 0..128), present_id in any::<bool>(), present_data in any::<bool>(), flags in any::<u32>()) {
        v.push(0);
        let before = v.clone();
        check(present_id.then_some(v.as_slice()), present_data, flags);
        prop_assert_eq!(v, before);
    }
}
