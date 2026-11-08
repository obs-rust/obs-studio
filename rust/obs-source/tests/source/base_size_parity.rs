//! Real-header field layout and original-C differential traversal. No exclusions.
use obs_source::ffi::source_base_size::*;
use proptest::prelude::*;
use std::ffi::c_void;
unsafe extern "C" {
    fn oracle_source_base_size_new(values: *const u32) -> *mut c_void;
    fn oracle_source_base_size_free(handle: *mut c_void);
    fn oracle_source_base_size_unchanged(handle: *const c_void) -> bool;
    fn oracle_source_base_size_layout(layout: *mut usize);
    fn oracle_obs_source_get_base_width(source: *const c_void) -> u32;
    fn oracle_obs_source_get_base_height(source: *const c_void) -> u32;
    fn oracle_obs_source_base_width(source: *const c_void) -> u32;
    fn oracle_obs_source_base_height(source: *const c_void) -> u32;
}
struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: owned fixture returned by the matching C allocator.
        unsafe {
            oracle_source_base_size_free(self.0);
        }
    }
}
fn check(values: [u32; 24]) {
    // SAFETY: input array is readable; the returned fixture owns three linked
    // sources and all their callback data until Drop. Neither getter retains it.
    unsafe {
        let fixture = Fixture(oracle_source_base_size_new(values.as_ptr()));
        assert!(!fixture.0.is_null());
        assert_eq!(
            obs_source_get_base_width(fixture.0),
            oracle_obs_source_get_base_width(fixture.0)
        );
        assert_eq!(
            obs_source_get_base_height(fixture.0),
            oracle_obs_source_get_base_height(fixture.0)
        );
        assert_eq!(
            obs_source_base_width(fixture.0),
            oracle_obs_source_base_width(fixture.0)
        );
        assert_eq!(
            obs_source_base_height(fixture.0),
            oracle_obs_source_base_height(fixture.0)
        );
        assert!(oracle_source_base_size_unchanged(fixture.0));
    }
}
#[test]
fn characterized_edges() {
    check([0; 24]);
    check([u32::MAX; 24]);
    for kind in 0..3 {
        for flags in 0..128 {
            let mut values = [0; 24];
            for chunk in values.chunks_mut(8) {
                chunk.copy_from_slice(&[kind, flags, 1920, 1080, u32::MAX, 480, 270, 17]);
            }
            check(values);
        }
    }
    // SAFETY: public NULL queries are defined and never reach a field accessor.
    unsafe {
        assert_eq!(obs_source_get_base_width(std::ptr::null()), 0);
        assert_eq!(obs_source_get_base_height(std::ptr::null()), 0);
    }
}
proptest! {
    #![proptest_config(ProptestConfig {cases: obs_c_oracle::parity::cases(128), failure_persistence: None, ..ProptestConfig::default()})]
    #[test]
    fn original_c_parity(values in any::<[u32;24]>()) { check(values); }
}
#[test]
fn real_header_layout() {
    let mut actual = [0; 15];
    // SAFETY: the array has one entry per C layout measurement.
    unsafe {
        oracle_source_base_size_layout(actual.as_mut_ptr());
    }
    assert_eq!(
        actual,
        [
            size_of::<BaseSizeFields>(),
            align_of::<BaseSizeFields>(),
            std::mem::offset_of!(BaseSizeFields, source_type),
            std::mem::offset_of!(BaseSizeFields, data),
            std::mem::offset_of!(BaseSizeFields, get_width),
            std::mem::offset_of!(BaseSizeFields, get_height),
            std::mem::offset_of!(BaseSizeFields, filter_parent),
            std::mem::offset_of!(BaseSizeFields, filter_target),
            std::mem::offset_of!(BaseSizeFields, enabled),
            std::mem::offset_of!(BaseSizeFields, async_active),
            std::mem::offset_of!(BaseSizeFields, async_rotation),
            std::mem::offset_of!(BaseSizeFields, transition_width),
            std::mem::offset_of!(BaseSizeFields, transition_height),
            std::mem::offset_of!(BaseSizeFields, async_width),
            std::mem::offset_of!(BaseSizeFields, async_height)
        ]
    );
}
