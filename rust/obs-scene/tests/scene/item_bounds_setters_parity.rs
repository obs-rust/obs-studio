//! Full-item byte parity against the extracted C setters; no behavior exclusions.
use obs_graphics::ffi::vec2::vec2;
use obs_scene::ffi::{item_bounds_getters::SceneItemPrefix, item_bounds_setters::*};
use proptest::prelude::*;
type TypeSetter = unsafe extern "C" fn(*mut SceneItemPrefix, u32);
type CropSetter = unsafe extern "C" fn(*mut SceneItemPrefix, bool);
type SizeSetter = unsafe extern "C" fn(*mut SceneItemPrefix, *const vec2);
type Validator = extern "C" fn(i64) -> u32;
unsafe extern "C" {
    fn oracle_scene_item_bounds_setters_probe(
        kind: u32,
        align: u32,
        flags: u32,
        x: f32,
        y: f32,
        height: u32,
        set_type: TypeSetter,
        set_align: TypeSetter,
        set_crop: CropSetter,
        set_size: SizeSetter,
        validate: Validator,
    ) -> u32;
}
fn check(kind: u32, align: u32, flags: u32, x: f32, y: f32, height: u32) {
    // SAFETY: C owns complete live fixtures, recursive mutexes and readable sizes.
    let result = unsafe {
        oracle_scene_item_bounds_setters_probe(
            kind,
            align,
            flags,
            x,
            y,
            height,
            obs_sceneitem_set_bounds_type,
            obs_sceneitem_set_bounds_alignment,
            obs_sceneitem_set_bounds_crop,
            obs_sceneitem_set_bounds,
            obs_scene_bounds_valid_type,
        )
    };
    assert_eq!(
        result, 0,
        "kind={kind} alignment={align:#x} flags={flags:#x} size=({x},{y}) height={height}"
    );
}
#[test]
fn characterized_edges() {
    for flags in 0..128 {
        for kind in [0, 1, 2, 3, 4, 5, 6, 7, u32::MAX, i32::MAX as u32] {
            check(kind, u32::MAX, flags, 540.0, -270.0, 1080);
        }
    }
    for height in [0, 1, 720, u32::MAX] {
        check(3, 5, 10, -20.0, 35.5, height);
    }
}
proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(128), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn setters_match_c(kind in any::<u32>(), align in any::<u32>(), flags in 0_u32..128,
        x in -100000.0_f32..100000.0, y in -100000.0_f32..100000.0, height in any::<u32>()) {
        check(kind, align, flags, x, y, height);
    }
}
