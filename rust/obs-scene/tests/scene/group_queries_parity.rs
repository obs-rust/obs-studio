//! Exact stored-flag parity against the original C; no exclusions.
use core::ffi::c_void;
use core::mem::{align_of, size_of};
use obs_scene::ffi::group_queries::{obs_scene_is_group, obs_sceneitem_is_group};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_scene_group_layout(layout: *mut usize);
    fn oracle_scene_group_new(bits: u32) -> *mut c_void;
    fn oracle_scene_group_item(handle: *mut c_void) -> *mut c_void;
    fn oracle_scene_group_unchanged(handle: *const c_void) -> bool;
    fn oracle_scene_group_free(handle: *mut c_void);
    fn oracle_obs_scene_is_group(scene: *const c_void) -> bool;
    fn oracle_obs_sceneitem_is_group(item: *mut c_void) -> bool;
}

#[test]
fn real_header_field_layout() {
    let mut layout = [0; 3];
    // SAFETY: C writes exactly three field size/alignment measurements.
    unsafe { oracle_scene_group_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [size_of::<bool>(), size_of::<bool>(), align_of::<bool>()]
    );
}

fn check(bits: u32) {
    // SAFETY: C owns real zero-initialized objects and snapshots every byte.
    // The scene is the fixture's first member. Both stay live until free.
    unsafe {
        let scene = oracle_scene_group_new(bits);
        assert!(!scene.is_null());
        let item = oracle_scene_group_item(scene);
        assert_eq!(obs_scene_is_group(scene), oracle_obs_scene_is_group(scene));
        assert_eq!(
            obs_sceneitem_is_group(item),
            oracle_obs_sceneitem_is_group(item)
        );
        assert_eq!(obs_scene_is_group(scene), bits & 2 != 0);
        assert_eq!(obs_sceneitem_is_group(item), bits & 1 != 0);
        assert!(oracle_scene_group_unchanged(scene));
        oracle_scene_group_free(scene);
    }
}

#[test]
fn characterized_edges() {
    // SAFETY: public getters explicitly accept null; private accessors do not.
    unsafe {
        let absent = core::ptr::null_mut();
        assert_eq!(
            obs_scene_is_group(absent),
            oracle_obs_scene_is_group(absent)
        );
        assert_eq!(
            obs_sceneitem_is_group(absent),
            oracle_obs_sceneitem_is_group(absent)
        );
        assert!(!obs_scene_is_group(absent));
        assert!(!obs_sceneitem_is_group(absent));
    }
    for bits in 0..16 {
        check(bits);
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(128), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn flags_match_c(bits in any::<u32>()) {
        check(bits);
    }
}
