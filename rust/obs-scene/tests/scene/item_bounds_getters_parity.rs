//! Complete C-owned items and exact scalar/state parity; no intended differences.
//! Float neighbors are opaque and must retain every bit, including NaN payloads.
use core::mem::{align_of, offset_of, size_of};
use obs_c_oracle as _;
use obs_scene::ffi::item_bounds_getters::{SceneItemPrefix, obs_sceneitem_get_bounds_type,
    obs_sceneitem_get_bounds_alignment, obs_sceneitem_get_bounds_crop};
use proptest::prelude::*;

type TypeGetter = unsafe extern "C" fn(*const SceneItemPrefix) -> u32;
type CropGetter = unsafe extern "C" fn(*const SceneItemPrefix) -> bool;
unsafe extern "C" {
    fn oracle_scene_item_bounds_layout(field: u32) -> usize;
    fn oracle_scene_item_bounds_probe(kind: u32, alignment: u32, flags: u32, neighbor: u32,
        get_type: TypeGetter, get_align: TypeGetter, get_crop: CropGetter) -> u32;
    fn oracle_scene_item_bounds_concurrent(get_type: TypeGetter, get_align: TypeGetter,
        get_crop: CropGetter) -> u32;
}

#[test]
fn real_header_prefix_layout() {
    let rust = [size_of::<SceneItemPrefix>(), align_of::<SceneItemPrefix>(),
        offset_of!(SceneItemPrefix, state),
        offset_of!(SceneItemPrefix, item_render),
        offset_of!(SceneItemPrefix, crop),
        offset_of!(SceneItemPrefix, absolute_coordinates),
        offset_of!(SceneItemPrefix, pos),
        offset_of!(SceneItemPrefix, scale),
        offset_of!(SceneItemPrefix, scale_ref),
        offset_of!(SceneItemPrefix, rot),
        offset_of!(SceneItemPrefix, align),
        offset_of!(SceneItemPrefix, last_width),
        offset_of!(SceneItemPrefix, last_height),
        offset_of!(SceneItemPrefix, output_scale),
        offset_of!(SceneItemPrefix, scale_filter),
        offset_of!(SceneItemPrefix, blend_method),
        offset_of!(SceneItemPrefix, blend_type),
        offset_of!(SceneItemPrefix, box_transform),
        offset_of!(SceneItemPrefix, box_scale),
        offset_of!(SceneItemPrefix, draw_transform),
        offset_of!(SceneItemPrefix, bounds_type),
        offset_of!(SceneItemPrefix, bounds_align),
        offset_of!(SceneItemPrefix, bounds),
        offset_of!(SceneItemPrefix, crop_to_bounds),
        size_of::<u32>(), size_of::<u32>(), size_of::<u32>(), size_of::<u32>()];
    for (index, value) in rust.into_iter().enumerate() {
        // SAFETY: accepts any integer, returning a real-header layout measurement.
        assert_eq!(value, unsafe { oracle_scene_item_bounds_layout(index as u32) }, "layout index {index}");
    }
}

fn check(kind: u32, alignment: u32, flags: u32, neighbor: u32) {
    // SAFETY: C owns complete live items and parents, initializes each read field,
    // and supplies their addresses only while alive. Callbacks preserve C locks.
    let mismatch = unsafe { oracle_scene_item_bounds_probe(kind, alignment, flags, neighbor,
        obs_sceneitem_get_bounds_type, obs_sceneitem_get_bounds_alignment, obs_sceneitem_get_bounds_crop) };
    assert_eq!(mismatch, 0, "kind={kind}, alignment={alignment:#x}, flags={flags}, neighbor={neighbor:#x}, mismatch bits={mismatch}");
}

#[test]
fn characterized_edges() {
    for kind in 0..=6 {
        for flags in 0..8 {
            for alignment in [0, 1, 2, 4, 8, 16, 0x8000_0000, u32::MAX - kind, u32::MAX] {
                for neighbor in [0, 0x8000_0000, 0x7f80_0000, 0xff80_0000, 0x7fc0_1234, 0x7fa0_4321, 1] {
                    check(kind, alignment, flags, neighbor);
                }
            }
        }
    }
}

#[test]
fn parent_video_mutex_blocks_and_publishes() {
    // SAFETY: the fixture joins each worker before destroying complete C-owned
    // items, mutexes and events. Each callback follows the original C contract.
    assert_eq!(unsafe { oracle_scene_item_bounds_concurrent(obs_sceneitem_get_bounds_type,
        obs_sceneitem_get_bounds_alignment, obs_sceneitem_get_bounds_crop) }, 0);
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(1024),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn arbitrary_fields(kind in 0u32..=6, alignment in any::<u32>(), flags in any::<u32>(), neighbor in any::<u32>()) {
        check(kind, alignment, flags, neighbor);
    }
}
