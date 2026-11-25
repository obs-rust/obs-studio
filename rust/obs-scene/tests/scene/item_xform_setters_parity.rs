//! Tier 3 original C versus Rust, whole-item bytes and mutex-observed callbacks.
//! Existing bounds/transform layout tests measure the reused prefix in full.
//! No intentional differences; invalid non-null vectors remain caller UB in C.
use obs_graphics::ffi::vec2::vec2;
use obs_scene::ffi::{
    item_bounds_getters::SceneItemPrefix,
    item_xform_setters::{obs_sceneitem_set_pos, obs_sceneitem_set_rot, obs_sceneitem_set_scale},
};
use proptest::prelude::*;
type Setter = unsafe extern "C" fn(*mut SceneItemPrefix, *const vec2);
unsafe extern "C" {
    fn oracle_scene_xform_probe(
        setter: Setter,
        which: u32,
        value: *const f32,
        defer: i32,
        flags: u32,
    ) -> u32;
}
unsafe extern "C" fn rotation(item: *mut SceneItemPrefix, value: *const vec2) {
    // SAFETY: the oracle supplies a live vector and item for the callback.
    unsafe { obs_sceneitem_set_rot(item, (*value).x) };
}
fn check(which: u32, value: [f32; 2], defer: i32, flags: u32) {
    let setters: [Setter; 3] = [obs_sceneitem_set_pos, rotation, obs_sceneitem_set_scale];
    // SAFETY: the C oracle owns full live items/scenes and passes valid pointers.
    assert_eq!(
        unsafe {
            oracle_scene_xform_probe(setters[which as usize], which, value.as_ptr(), defer, flags)
        },
        0,
        "which={which} value={value:?} defer={defer} flags={flags}"
    );
}
#[test]
fn characterized_paths_aliases_and_float_bits() {
    for which in 0..3 {
        for flags in [0, 1, 4, 5, 9, 17, 33, 127] {
            for (index, value) in [
                [0., -0.],
                [-2., 3.5],
                [480., 810.],
                [450., -720.],
                [f32::INFINITY, f32::NEG_INFINITY],
                [f32::from_bits(0x7fc12345), f32::MIN_POSITIVE],
            ]
            .into_iter()
            .enumerate()
            {
                // Tier 1 and Tier 2 exhaust the defer cross-product. Rotate
                // it here to keep the independent mutex probes inexpensive.
                let defer = [-1, 0, 1, 3][index % 4];
                check(which, value, defer, flags);
            }
        }
    }
}
#[test]
fn null_items_never_read_vectors() {
    // SAFETY: all three C setters explicitly accept NULL items; vectors are not read.
    unsafe {
        obs_sceneitem_set_pos(core::ptr::null_mut(), core::ptr::null());
        obs_sceneitem_set_rot(core::ptr::null_mut(), f32::NAN);
        obs_sceneitem_set_scale(core::ptr::null_mut(), core::ptr::null());
    }
}
proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256),failure_persistence:None,..ProptestConfig::default() })]
    #[test]
    fn generated_field_bytes_and_callback_order(which in 0..3u32, x in any::<u32>(), y in any::<u32>(),defer in any::<i32>(),flags in 0..128u32) {
        check(which,[f32::from_bits(x),f32::from_bits(y)],defer,flags);
    }
}
