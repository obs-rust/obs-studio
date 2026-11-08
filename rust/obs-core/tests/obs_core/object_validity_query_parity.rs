//! Real-header scalar layout and read-only validity parity. No exclusions.
use core::ffi::c_void;
use core::mem::{align_of, size_of};
use obs_core::ffi::object_validity_query::obs_obj_invalid;
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_object_validity_layout(layout: *mut usize);
    fn oracle_object_validity_new(
        kind: u32,
        privacy: bool,
        populated: bool,
        payload: u64,
    ) -> *mut c_void;
    fn oracle_object_validity_unchanged(obj: *const c_void) -> bool;
    fn oracle_object_validity_free(obj: *mut c_void);
    fn oracle_obs_obj_invalid(obj: *mut c_void) -> bool;
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 6];
    // SAFETY: the oracle writes exactly six scalar layout values.
    unsafe { oracle_object_validity_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<*mut c_void>(),
            align_of::<*mut c_void>(),
            size_of::<*const *mut c_void>(),
            align_of::<*const *mut c_void>(),
            size_of::<bool>(),
            align_of::<bool>()
        ]
    );
}

fn check(kind: u32, privacy: bool, populated: bool, payload: u64) {
    // SAFETY: C allocates a real context as its first member. Both queries read
    // it without mutation; the fixture remains live until its final free.
    unsafe {
        let obj = oracle_object_validity_new(kind, privacy, populated, payload);
        assert!(!obj.is_null());
        assert_eq!(obs_obj_invalid(obj), oracle_obs_obj_invalid(obj));
        assert_eq!(obs_obj_invalid(obj), !populated);
        assert!(oracle_object_validity_unchanged(obj));
        oracle_object_validity_free(obj);
    }
}

#[test]
fn characterized_edges() {
    // SAFETY: NULL is explicitly accepted by the original query contract.
    unsafe {
        assert_eq!(
            obs_obj_invalid(core::ptr::null_mut()),
            oracle_obs_obj_invalid(core::ptr::null_mut())
        );
        assert!(obs_obj_invalid(core::ptr::null_mut()));
    }
    for kind in [0, 1, 2, 3, 4, 5, 6, i32::MAX as u32, 1 << 31, u32::MAX] {
        for privacy in [false, true] {
            for populated in [false, true] {
                for payload in [0, 42, u64::MAX] {
                    check(kind, privacy, populated, payload);
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(512), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn query_matches_c(kind in any::<u32>(), privacy in any::<bool>(), populated in any::<bool>(), payload in any::<u64>()) {
        check(kind, privacy, populated, payload);
    }
}
