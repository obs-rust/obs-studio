//! Exact borrowed-pointer parity against the original C; no exclusions.
use core::ffi::c_void;
use core::mem::{align_of, size_of};
use obs_scene::ffi::item_parent_getter::obs_sceneitem_get_scene;
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_scene_parent_layout(layout: *mut usize);
    fn oracle_scene_parent_new(bits: u32) -> *mut c_void;
    fn oracle_scene_parent_unchanged(handle: *const c_void) -> bool;
    fn oracle_scene_parent_free(handle: *mut c_void);
    fn oracle_obs_sceneitem_get_scene(item: *const c_void) -> *mut c_void;
}

#[test]
fn real_header_parent_field_layout() {
    let mut layout = [0; 2];
    // SAFETY: C writes exactly two real-header pointer size/alignment values.
    unsafe { oracle_scene_parent_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [size_of::<*mut c_void>(), align_of::<*mut c_void>()]
    );
}

fn check(bits: u32) {
    // SAFETY: C owns the real item (the fixture's first member) and two parents.
    // The original and Rust getters only borrow them; all stay live until free.
    unsafe {
        let item = oracle_scene_parent_new(bits);
        assert!(!item.is_null());
        let expected = oracle_obs_sceneitem_get_scene(item);
        let actual = obs_sceneitem_get_scene(item);
        assert_eq!(actual, expected);
        assert_eq!(actual.is_null(), bits & 4 == 0);
        assert_eq!(obs_sceneitem_get_scene(item), expected);
        assert!(oracle_scene_parent_unchanged(item));
        oracle_scene_parent_free(item);
    }
}

#[test]
fn characterized_edges() {
    // SAFETY: public getters accept NULL; the field accessor is never called on it.
    unsafe {
        let absent = core::ptr::null();
        assert_eq!(
            obs_sceneitem_get_scene(absent),
            oracle_obs_sceneitem_get_scene(absent)
        );
        assert!(obs_sceneitem_get_scene(absent).is_null());
    }
    for bits in 0..16 {
        check(bits);
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(128), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn parent_matches_c(bits in any::<u32>()) {
        check(bits);
    }
}
