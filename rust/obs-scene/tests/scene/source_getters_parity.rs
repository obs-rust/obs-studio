//! Exact borrowed-pointer parity against the original C; no exclusions.
use core::ffi::c_void;
use core::mem::{align_of, size_of};
use obs_scene::ffi::source_getters::{obs_scene_get_source, obs_sceneitem_get_source};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_scene_source_layout(layout: *mut usize);
    fn oracle_scene_source_new(bits: u32) -> *mut c_void;
    fn oracle_scene_source_item(handle: *mut c_void) -> *mut c_void;
    fn oracle_scene_source_unchanged(handle: *const c_void) -> bool;
    fn oracle_scene_source_free(handle: *mut c_void);
    fn oracle_obs_scene_get_source(scene: *const c_void) -> *mut c_void;
    fn oracle_obs_sceneitem_get_source(item: *const c_void) -> *mut c_void;
}

#[test]
fn real_header_source_field_layout() {
    let mut layout = [0; 4];
    // SAFETY: C writes exactly four real-header pointer size/alignment values.
    unsafe { oracle_scene_source_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<*mut c_void>(),
            align_of::<*mut c_void>(),
            size_of::<*mut c_void>(),
            align_of::<*mut c_void>()
        ]
    );
}

fn check(bits: u32) {
    // SAFETY: C owns the real scene (the fixture's first member), item and sources.
    // The original and Rust getters borrow them; all stay live until free.
    unsafe {
        let scene = oracle_scene_source_new(bits);
        assert!(!scene.is_null());
        let item = oracle_scene_source_item(scene);
        let expected_scene = oracle_obs_scene_get_source(scene);
        let expected_item = oracle_obs_sceneitem_get_source(item);
        assert_eq!(obs_scene_get_source(scene), expected_scene);
        assert_eq!(obs_sceneitem_get_source(item), expected_item);
        assert_eq!(expected_scene.is_null(), bits & 1 == 0);
        assert_eq!(expected_item.is_null(), bits & 2 == 0);
        assert_eq!(obs_scene_get_source(scene), expected_scene);
        assert_eq!(obs_sceneitem_get_source(item), expected_item);
        assert!(oracle_scene_source_unchanged(scene));
        oracle_scene_source_free(scene);
    }
}

#[test]
fn characterized_edges() {
    // SAFETY: public getters accept NULL; field accessors are never called on it.
    unsafe {
        let absent = core::ptr::null();
        assert_eq!(
            obs_scene_get_source(absent),
            oracle_obs_scene_get_source(absent)
        );
        assert_eq!(
            obs_sceneitem_get_source(absent),
            oracle_obs_sceneitem_get_source(absent)
        );
        assert!(obs_scene_get_source(absent).is_null());
        assert!(obs_sceneitem_get_source(absent).is_null());
    }
    for bits in 0..16 {
        check(bits);
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(128), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn sources_match_c(bits in any::<u32>()) {
        check(bits);
    }
}
