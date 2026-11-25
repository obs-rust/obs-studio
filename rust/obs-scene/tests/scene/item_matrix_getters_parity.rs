//! Original-C differential copies of complete items, arbitrary float bits,
//! sentinel guards, parented/detached and recursive-lock paths. No differences.
use obs_c_oracle as _;
use obs_graphics::ffi::{matrix4::matrix4, vec2::vec2};
use obs_scene::ffi::{item_bounds_getters::SceneItemPrefix, item_matrix_getters::*};
use proptest::prelude::*;

type MatrixGetter = unsafe extern "C" fn(*const SceneItemPrefix, *mut matrix4);
type ScaleGetter = unsafe extern "C" fn(*const SceneItemPrefix, *mut vec2);
unsafe extern "C" {
    fn oracle_scene_item_matrix_probe(
        bits: *const u32, flags: u32, draw: MatrixGetter, box_get: MatrixGetter, scale: ScaleGetter,
    ) -> u32;
}

fn check(bits: [u32; 34], flags: u32) {
    // SAFETY: C owns complete items, live parent mutexes and guarded outputs,
    // and invokes these callbacks only within those storage lifetimes.
    assert_eq!(unsafe {
        oracle_scene_item_matrix_probe(bits.as_ptr(), flags,
            obs_sceneitem_get_draw_transform, obs_sceneitem_get_box_transform,
            obs_sceneitem_get_box_scale)
    }, 0, "flags={flags} bits={bits:08x?}");
}

#[test]
fn characterized_float_edges() {
    let edges = [0, 0x80000000, 0x7fc00001, 0xffc12345, 0x7f800000,
        0xff800000, 1, 0x3f800000, 0x7f801234];
    for start in 0..edges.len() {
        for flags in 0..4 {
            check(core::array::from_fn(|i| edges[(start + i) % edges.len()]), flags);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256), failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn arbitrary_bits(bits in prop::array::uniform(any::<u32>()), flags in 0u32..4) {
        check(bits, flags);
    }
}

// Reused matrix4/vec2 layouts are covered field by field in the Graphics
// category; the complete SceneItemPrefix size/alignment and every field offset
// (including these three fields) are checked by item_bounds_getters_parity.
