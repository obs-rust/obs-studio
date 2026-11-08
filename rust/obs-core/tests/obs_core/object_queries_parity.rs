//! Real-header layout and exact read-only query parity. No exclusions.
use core::ffi::{c_int, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_core::ffi::object_queries::{Fields, obs_obj_get_type, obs_obj_is_private};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_object_query_layout(layout: *mut usize);
    fn oracle_object_query_new(kind: u32, privacy: bool) -> *mut c_void;
    fn oracle_object_query_unchanged(obj: *const c_void) -> bool;
    fn oracle_object_query_free(obj: *mut c_void);
    fn oracle_obs_obj_get_type(obj: *mut c_void) -> c_int;
    fn oracle_obs_obj_is_private(obj: *mut c_void) -> bool;
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 10];
    // SAFETY: the oracle writes exactly ten layout and enum-constant values.
    unsafe { oracle_object_query_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, kind),
            offset_of!(Fields, private),
            size_of::<c_int>(),
            align_of::<c_int>(),
            size_of::<bool>(),
            align_of::<bool>(),
            0,
            5
        ]
    );
}

fn check(kind: u32, privacy: bool) {
    // SAFETY: C allocates a real context as the fixture's first member; both
    // getters read it without mutation, and it stays live until the final free.
    unsafe {
        let obj = oracle_object_query_new(kind, privacy);
        assert!(!obj.is_null());
        assert_eq!(obs_obj_get_type(obj), oracle_obs_obj_get_type(obj));
        assert_eq!(obs_obj_get_type(obj), kind as i32);
        assert_eq!(obs_obj_is_private(obj), oracle_obs_obj_is_private(obj));
        assert_eq!(obs_obj_is_private(obj), privacy);
        assert!(oracle_object_query_unchanged(obj));
        oracle_object_query_free(obj);
    }
}

#[test]
fn characterized_edges() {
    // SAFETY: NULL is explicitly accepted by both original query contracts.
    unsafe {
        let obj = core::ptr::null_mut();
        assert_eq!(obs_obj_get_type(obj), oracle_obs_obj_get_type(obj));
        assert_eq!(obs_obj_get_type(obj), 0);
        assert_eq!(obs_obj_is_private(obj), oracle_obs_obj_is_private(obj));
        assert!(!obs_obj_is_private(obj));
    }
    for kind in [0, 1, 2, 3, 4, 5, 6, 127, i32::MAX as u32, 1 << 31, u32::MAX] {
        for privacy in [false, true] {
            check(kind, privacy);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(512), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn queries_match_c(kind in any::<u32>(), privacy in any::<bool>()) {
        check(kind, privacy);
    }
}
