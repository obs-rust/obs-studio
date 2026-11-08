//! Tier 2 layout and Tier 3 borrowed-pointer, state and diagnostic parity.
//! No input exclusions; oracle-only __func__ symbol prefixes are normalized.
use core::ffi::{c_char, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_source::ffi::source_identity_getters::{
    Fields, obs_source_get_unversioned_id as unversioned_id, obs_source_get_uuid as uuid,
};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*const c_void) -> *const c_char;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_source_identity_new(
        uuid: *const c_char,
        unversioned_id: *const c_char,
        flags: u32,
    ) -> *mut c_void;
    fn oracle_source_identity_free(source: *mut c_void);
    fn oracle_source_identity_unchanged(source: *const c_void) -> bool;
    fn oracle_source_identity_addresses(source: *const c_void) -> bool;
    fn oracle_source_identity_layout(layout: *mut usize);
    fn oracle_obs_source_get_uuid(source: *const c_void) -> *const c_char;
    fn oracle_obs_source_get_unversioned_id(source: *const c_void) -> *const c_char;
    fn oracle_source_identity_logs(
        uuid: Getter,
        unversioned_id: Getter,
        get: GetLog,
        set: SetLog,
    ) -> bool;
}

struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: owns the C allocation; borrowed metadata is still live here.
        unsafe { oracle_source_identity_free(self.0) }
    }
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 6];
    // SAFETY: the oracle writes exactly six size/offset values.
    unsafe { oracle_source_identity_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, uuid),
            offset_of!(Fields, unversioned_id),
            size_of::<*const c_char>(),
            align_of::<*const c_char>()
        ]
    );
}

fn check(v: Option<&[u8]>, a: Option<&[u8]>, flags: u32) {
    let vp = v.map_or(core::ptr::null(), |s| s.as_ptr().cast());
    let ap = a.map_or(core::ptr::null(), |s| s.as_ptr().cast());
    // SAFETY: the C fixture only borrows these byte buffers and never decodes them.
    let fixture = Fixture(unsafe { oracle_source_identity_new(vp, ap, flags) });
    assert!(!fixture.0.is_null());
    // SAFETY: the real-layout fixture stays live; only public getters receive
    // null in the separate diagnostic test, never a member-address accessor.
    unsafe {
        assert!(oracle_source_identity_addresses(fixture.0));
        for _ in 0..2 {
            assert_eq!(uuid(fixture.0), oracle_obs_source_get_uuid(fixture.0));
            assert_eq!(
                unversioned_id(fixture.0),
                oracle_obs_source_get_unversioned_id(fixture.0)
            );
            assert_eq!(uuid(fixture.0), vp);
            assert_eq!(unversioned_id(fixture.0), ap);
            assert!(oracle_source_identity_unchanged(fixture.0));
        }
    }
}

#[test]
fn characterized_edges() {
    let values: [Option<&[u8]>; 4] = [None, Some(b"\0"), Some(b"h264;\xff\0"), Some(b"aac;opus\0")];
    for (v, value_v) in values.iter().enumerate() {
        for (a, value_a) in values.iter().enumerate() {
            check(*value_v, *value_a, (v * 4 + a) as u32);
        }
    }
    check(Some(b"same\0"), Some(b"same\0"), u32::MAX);
}

#[test]
fn null_diagnostics_match_c() {
    if std::env::var_os("OBS_SOURCE_IDENTITY_CHILD").is_none() {
        let selector = concat!(module_path!(), "::null_diagnostics_match_c")
            .split_once("::")
            .unwrap()
            .1;
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", selector, "--nocapture"])
            .env("OBS_SOURCE_IDENTITY_CHILD", "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    // SAFETY: the isolated child exclusively owns the log handler; callbacks
    // use the base shim's opaque handler-pointer ABI, and the probe restores it.
    unsafe {
        assert!(oracle_source_identity_logs(
            uuid,
            unversioned_id,
            base_get_log_handler,
            base_set_log_handler
        ));
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn arbitrary_metadata_matches_c(mut v in prop::collection::vec(any::<u8>(), 0..128), mut a in prop::collection::vec(any::<u8>(), 0..128), present_v in any::<bool>(), present_a in any::<bool>(), flags in any::<u32>()) {
        v.push(0); a.push(0);
        let before_v = v.clone(); let before_a = a.clone();
        check(present_v.then_some(v.as_slice()), present_a.then_some(a.as_slice()), flags);
        prop_assert_eq!(v, before_v); prop_assert_eq!(a, before_a);
    }
}
