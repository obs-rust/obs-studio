//! Tier 3: exact predicate and no-ownership-change parity, with no exclusions.
//! Even expired fixtures retain their source allocations until after probing.
use core::ffi::{c_long, c_void};
use core::mem::{align_of, size_of};
use obs_source::ffi::weak_predicates::{
    obs_weak_source_expired as expired, obs_weak_source_references_source as references,
};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_source_weak_predicates_new(
        owners: c_long,
        weak_owners: c_long,
        stored: u32,
    ) -> *mut c_void;
    fn oracle_source_weak_predicates_target(handle: *mut c_void, target: u32) -> *mut c_void;
    fn oracle_source_weak_predicates_unchanged(handle: *const c_void) -> bool;
    fn oracle_source_weak_predicates_addresses(weak: *const c_void) -> bool;
    fn oracle_source_weak_predicates_long_layout(layout: *mut usize);
    fn oracle_source_weak_predicates_free(handle: *mut c_void);
    fn oracle_obs_weak_source_expired(weak: *mut c_void) -> bool;
    fn oracle_obs_weak_source_references_source(weak: *mut c_void, source: *mut c_void) -> bool;
}

struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: owns the allocation, with no outstanding predicate calls.
        unsafe { oracle_source_weak_predicates_free(self.0) }
    }
}

fn check(owners: c_long, weak_owners: c_long, stored: u32, target: u32, present: bool) {
    // SAFETY: indices are in 0..=2; count fields are initialized before use.
    let fixture =
        Fixture(unsafe { oracle_source_weak_predicates_new(owners, weak_owners, stored) });
    assert!(!fixture.0.is_null());
    // SAFETY: all returned identities point into this live fixture or are NULL.
    let source = unsafe { oracle_source_weak_predicates_target(fixture.0, target) };
    let weak = if present {
        fixture.0
    } else {
        core::ptr::null_mut()
    };
    // SAFETY: the first fixture member is the real weak-source control block;
    // neither implementation dereferences source, and every allocation is live.
    unsafe {
        assert!(oracle_source_weak_predicates_addresses(fixture.0));
        for _ in 0..3 {
            assert_eq!(expired(weak), oracle_obs_weak_source_expired(weak));
            assert_eq!(
                references(weak, source),
                oracle_obs_weak_source_references_source(weak, source)
            );
            assert_eq!(expired(weak), !present || owners < 0);
            assert_eq!(
                references(weak, source),
                present && stored != 0 && target != 0 && stored == target
            );
            assert!(oracle_source_weak_predicates_unchanged(fixture.0));
        }
    }
}

#[test]
fn native_long_matches_c_header() {
    let mut layout = [0; 2];
    // SAFETY: the oracle writes exactly two size/alignment entries.
    unsafe { oracle_source_weak_predicates_long_layout(layout.as_mut_ptr()) };
    assert_eq!(layout, [size_of::<c_long>(), align_of::<c_long>()]);
}

#[test]
fn nullable_expiry_and_identity_edges_match_c() {
    for owners in [c_long::MIN, -2, -1, 0, 1, c_long::MAX] {
        for stored in 0..=2 {
            for target in 0..=2 {
                for present in [false, true] {
                    check(owners, 0, stored, target, present);
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn arbitrary_weak_state_matches_c(
        owners in any::<c_long>(), weak_owners in 0..c_long::MAX,
        stored in 0u32..=2, target in 0u32..=2, present in any::<bool>(),
    ) {
        check(owners, weak_owners, stored, target, present);
    }
}
