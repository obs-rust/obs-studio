//! Exact original-C differential coverage; no intentional differences or exclusions.
use core::ffi::{c_char, c_void};
use core::mem::{align_of, size_of};
use obs_source::ffi::source_type_id_getters::{obs_source_get_id, obs_source_get_type};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type TypeGetter = unsafe extern "C" fn(*const c_void) -> u32;
type IdGetter = unsafe extern "C" fn(*const c_void) -> *const c_char;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_source_type_id_new(kind: u32, id: *const c_char, name: *const c_char) -> *mut c_void;
    fn oracle_source_type_id_free(source: *mut c_void);
    fn oracle_source_type_id_rename(source: *mut c_void, name: *const c_char);
    fn oracle_source_type_id_unchanged(source: *const c_void) -> bool;
    fn oracle_source_type_id_addresses(source: *const c_void) -> bool;
    fn oracle_source_type_id_layout(layout: *mut usize);
    fn oracle_obs_source_get_type(source: *const c_void) -> u32;
    fn oracle_obs_source_get_id(source: *const c_void) -> *const c_char;
    fn oracle_source_type_id_logs(
        kind: TypeGetter,
        id: IdGetter,
        get_log: GetLog,
        set_log: SetLog,
    ) -> bool;
}

struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: the fixture alone owns the C allocation; its strings are borrowed.
        unsafe { oracle_source_type_id_free(self.0) }
    }
}

#[test]
fn real_header_enum_layout() {
    let mut layout = [0; 6];
    // SAFETY: oracle writes exactly six enum representation and value measurements.
    unsafe { oracle_source_type_id_layout(layout.as_mut_ptr()) };
    assert_eq!(layout, [size_of::<u32>(), align_of::<u32>(), 0, 1, 2, 3]);
}

fn check(kind: u32, bytes: &[u8], present: bool) {
    let id = std::ffi::CString::new(bytes).unwrap();
    let borrowed = if present {
        id.as_ptr()
    } else {
        core::ptr::null()
    };
    // SAFETY: the C fixture borrows strings that outlive it and initializes real header fields.
    let fixture = Fixture(unsafe {
        oracle_source_type_id_new(kind, borrowed, c"original context name".as_ptr())
    });
    assert!(!fixture.0.is_null());
    // SAFETY: source is live, and both names and the ID remain alive until fixture drop.
    unsafe {
        assert!(oracle_source_type_id_addresses(fixture.0));
        for renamed in [false, true] {
            if renamed {
                oracle_source_type_id_rename(fixture.0, c"renamed context".as_ptr());
            }
            for _ in 0..2 {
                assert_eq!(
                    obs_source_get_type(fixture.0),
                    oracle_obs_source_get_type(fixture.0)
                );
                assert_eq!(obs_source_get_type(fixture.0), kind);
                assert_eq!(
                    obs_source_get_id(fixture.0),
                    oracle_obs_source_get_id(fixture.0)
                );
                assert_eq!(obs_source_get_id(fixture.0), borrowed);
                assert!(oracle_source_type_id_unchanged(fixture.0));
            }
        }
    }
}

#[test]
fn characterized_edges() {
    for (kind, id) in [
        (0, b"identity_input".as_slice()),
        (1, b"identity_filter"),
        (2, b"identity_transition"),
        (3, b"scene"),
    ] {
        check(kind, id, true);
        check(kind, id, false);
    }
    check(0, b"", true);
    check(2, &[255, 128, 1], true);
}

#[test]
fn null_diagnostics_match_c() {
    if !crate::output::isolated(concat!(module_path!(), "::null_diagnostics_match_c")) {
        return;
    }
    // SAFETY: the isolated child exclusively owns the process log handler; oracle restores it.
    unsafe {
        assert!(oracle_source_type_id_logs(
            obs_source_get_type,
            obs_source_get_id,
            base_get_log_handler,
            base_set_log_handler
        ));
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn arbitrary_identity_matches_c(kind in 0u32..=3, bytes in prop::collection::vec(1u8..=255, 0..128), present in any::<bool>()) {
        check(kind, &bytes, present);
    }
}
