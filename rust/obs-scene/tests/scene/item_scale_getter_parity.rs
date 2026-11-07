//! Bit-exact output and complete item preservation; no intended differences.
use core::mem::{align_of, offset_of, size_of};
use obs_c_oracle as _;
use obs_graphics::ffi::vec2::vec2;
use obs_scene::ffi::{item_bounds_getters::SceneItemPrefix, item_scale_getter::obs_sceneitem_get_scale};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*const SceneItemPrefix, *mut vec2);
unsafe extern "C" {
    fn oracle_scene_item_scale_layout(field: u32) -> usize;
    fn oracle_scene_item_scale_probe(x: u32, y: u32, reference: u32, flags: u32, getter: Getter) -> u32;
    fn oracle_scene_item_scale_concurrent(getter: Getter) -> u32;
}

#[test]
fn real_header_layout() {
    let values = [size_of::<vec2>(), align_of::<vec2>(), offset_of!(vec2, x), offset_of!(vec2, y),
        offset_of!(SceneItemPrefix, absolute_coordinates), offset_of!(SceneItemPrefix, scale),
        offset_of!(SceneItemPrefix, scale_ref)];
    // The complete reused item prefix and all its fields are independently
    // measured in item_bounds_getters_parity::real_header_prefix_layout.
    for (index, value) in values.into_iter().enumerate() {
        // SAFETY: the oracle only returns layout constants from the real header.
        assert_eq!(value, unsafe { oracle_scene_item_scale_layout(index as u32) });
    }
}

fn check(x: u32, y: u32, reference: u32, flags: u32) {
    // SAFETY: C owns complete live items, mutexes and guarded output storage,
    // and invokes the shim only during those objects' lifetimes.
    assert_eq!(unsafe { oracle_scene_item_scale_probe(x, y, reference, flags, obs_sceneitem_get_scale) },
        0, "x={x:08x} y={y:08x} reference={reference:08x} flags={flags}");
}

#[test]
fn characterized_edges() {
    let edges = [0, 0x80000000, 0x3f800000, 0xbf800000, 0x7f800000,
        0xff800000, 0x7fc01234, 0xffc05678, 0x7f801234, 1, 0x7f7fffff];
    for flags in 0..16 {
        for (index, &x) in edges.iter().enumerate() {
            for reference in [0.0f32, -0.0, 540.0, 1080.0, f32::INFINITY, f32::NAN] {
                check(x, edges[(index + 1) % edges.len()], reference.to_bits(), flags);
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(512), failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn arbitrary_bits(x: u32, y: u32, reference: u32, flags in 0u32..16) {
        check(x, y, reference, flags);
    }
}

#[test]
fn parent_video_mutex_blocks_and_publishes() {
    // SAFETY: the C fixture owns and joins each thread before freeing its items,
    // parent mutex and events. Four threads run sequentially, one at a time.
    assert_eq!(unsafe { oracle_scene_item_scale_concurrent(obs_sceneitem_get_scale) }, 0);
}
