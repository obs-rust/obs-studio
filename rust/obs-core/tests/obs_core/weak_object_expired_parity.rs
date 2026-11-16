//! Exact original-C parity over native-long counters; no input exclusions.
use core::ffi::{c_long, c_void};
use core::mem::{align_of, size_of};
use obs_core::ffi::weak_object_expired::obs_weak_object_expired as query;
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_weak_object_expired_new(
        owners: c_long,
        weak_refs: c_long,
        stored: bool,
    ) -> *mut c_void;
    fn oracle_weak_object_expired_unchanged(handle: *const c_void) -> bool;
    fn oracle_weak_object_expired_free(handle: *mut c_void);
    fn oracle_weak_object_expired_address(handle: *const c_void) -> bool;
    fn oracle_weak_object_expired_layout(layout: *mut usize);
    fn oracle_weak_object_expired_store(handle: *mut c_void, owners: c_long);
    fn oracle_obs_weak_object_expired(weak: *mut c_void) -> bool;
}

struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: this fixture owns the C allocation.
        unsafe { oracle_weak_object_expired_free(self.0) }
    }
}

#[test]
fn native_long_layout() {
    let mut layout = [0; 2];
    // SAFETY: oracle writes exactly two scalar layout values.
    unsafe { oracle_weak_object_expired_layout(layout.as_mut_ptr()) };
    assert_eq!(layout, [size_of::<c_long>(), align_of::<c_long>()]);
    if size_of::<c_long>() == 8 {
        assert_eq!(
            align_of::<c_long>(),
            align_of::<core::sync::atomic::AtomicI64>()
        );
    } else {
        assert_eq!(size_of::<c_long>(), 4);
        assert_eq!(
            align_of::<c_long>(),
            align_of::<core::sync::atomic::AtomicI32>()
        );
    }
}

fn check(owners: c_long, weak_refs: c_long, stored: bool, null: bool) {
    // SAFETY: oracle initializes the real C layout with arbitrary native-long counts.
    let fixture = Fixture(unsafe { oracle_weak_object_expired_new(owners, weak_refs, stored) });
    assert!(!fixture.0.is_null());
    // SAFETY: fixture stays live; only the public queries receive NULL.
    unsafe {
        assert!(oracle_weak_object_expired_address(fixture.0));
        let weak = if null {
            core::ptr::null_mut()
        } else {
            fixture.0
        };
        assert_eq!(query(weak), oracle_obs_weak_object_expired(weak));
        assert_eq!(query(weak), null || owners < 0);
        assert!(oracle_weak_object_expired_unchanged(fixture.0));
        // Re-read the same address after atomic C writes across the expiry boundary.
        for next in [0, -1, c_long::MAX, c_long::MIN] {
            oracle_weak_object_expired_store(fixture.0, next);
            assert_eq!(query(weak), oracle_obs_weak_object_expired(weak));
            assert_eq!(query(weak), null || next < 0);
            assert!(oracle_weak_object_expired_unchanged(fixture.0));
        }
    }
}

#[test]
fn mirrors_c_characterized_edges_without_mutation() {
    for owners in [c_long::MIN, -2, -1, 0, 1, c_long::MAX] {
        for weak_refs in [c_long::MIN, -2, -1, 0, 1, c_long::MAX] {
            for stored in [false, true] {
                for null in [false, true] {
                    check(owners, weak_refs, stored, null);
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn counters_match_c(owners in any::<c_long>(), weak_refs in any::<c_long>(), stored in any::<bool>(), null in any::<bool>()) {
        check(owners, weak_refs, stored, null);
    }
}
