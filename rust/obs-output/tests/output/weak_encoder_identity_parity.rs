//! Real-header layout and exact differential parity; no input exclusions.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::weak_encoder_identity::{
    Fields, obs_weak_encoder_references_encoder as query,
};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_weak_encoder_identity_new(stored: u32) -> *mut c_void;
    fn oracle_weak_encoder_identity_target(handle: *mut c_void, target: u32) -> *mut c_void;
    fn oracle_weak_encoder_identity_free(handle: *mut c_void);
    fn oracle_weak_encoder_identity_unchanged(handle: *const c_void) -> bool;
    fn oracle_weak_encoder_identity_addresses(handle: *const c_void) -> bool;
    fn oracle_weak_encoder_identity_layout(layout: *mut usize);
    fn oracle_obs_weak_encoder_references_encoder(weak: *mut c_void, encoder: *mut c_void) -> bool;
}

struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: owns the C allocation.
        unsafe { oracle_weak_encoder_identity_free(self.0) }
    }
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 3];
    // SAFETY: oracle writes exactly three size/offset values.
    unsafe { oracle_weak_encoder_identity_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, encoder)
        ]
    );
}

fn check(stored: u32, target: u32, null_weak: bool) {
    // SAFETY: bounded stored index initializes a real C weak handle and encoders.
    let fixture = Fixture(unsafe { oracle_weak_encoder_identity_new(stored) });
    assert!(!fixture.0.is_null());
    // SAFETY: fixture remains live; the address accessor only receives non-null.
    unsafe {
        assert!(oracle_weak_encoder_identity_addresses(fixture.0));
        let weak = if null_weak {
            core::ptr::null_mut()
        } else {
            fixture.0
        };
        let encoder = oracle_weak_encoder_identity_target(fixture.0, target);
        assert_eq!(
            query(weak, encoder),
            oracle_obs_weak_encoder_references_encoder(weak, encoder)
        );
        assert_eq!(
            query(weak, encoder),
            !null_weak && target != 0 && stored == target
        );
        assert!(oracle_weak_encoder_identity_unchanged(fixture.0));
    }
}

#[test]
fn characterized_edges() {
    for stored in 0..3 {
        for target in 0..3 {
            for null_weak in [false, true] {
                check(stored, target, null_weak);
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn identities_match_c(stored in 0u32..3, target in 0u32..3, null_weak in any::<bool>()) {
        check(stored, target, null_weak);
    }
}
