//! Exact scalar and complete-state parity; no intentional differences.
use core::mem::{align_of, offset_of, size_of};
use obs_c_oracle as _;
use obs_scene::ffi::{
    item_bounds_getters::SceneItemPrefix,
    item_transform_getters::{obs_sceneitem_get_alignment, obs_sceneitem_get_rot},
};
use proptest::prelude::*;
type RotGetter = unsafe extern "C" fn(*const SceneItemPrefix) -> f32;
type AlignGetter = unsafe extern "C" fn(*const SceneItemPrefix) -> u32;
unsafe extern "C" {
    fn oracle_scene_item_transform_getters_layout(field: u32) -> usize;
    fn oracle_scene_item_transform_probe(
        bits: u32,
        alignment: u32,
        flags: u32,
        get_rot: RotGetter,
        get_align: AlignGetter,
    ) -> u32;
    fn oracle_scene_item_transform_concurrent(get_rot: RotGetter, get_align: AlignGetter) -> u32;
}

#[test]
fn real_header_prefix_layout() {
    let rust = [
        size_of::<SceneItemPrefix>(),
        align_of::<SceneItemPrefix>(),
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
        size_of::<u32>(),
        size_of::<u32>(),
        size_of::<u32>(),
        size_of::<u32>(),
    ];
    for (index, value) in rust.into_iter().enumerate() {
        // SAFETY: accepts any integer, returning a real-header layout measurement.
        assert_eq!(
            value,
            unsafe { oracle_scene_item_transform_getters_layout(index as u32) },
            "layout index {index}"
        );
    }
}

fn check(bits: u32, alignment: u32, flags: u32) {
    // SAFETY: C owns complete live objects, initializes read fields, and joins
    // all workers before releasing objects. Callbacks preserve the C locks.
    let mismatch = unsafe {
        oracle_scene_item_transform_probe(
            bits,
            alignment,
            flags,
            obs_sceneitem_get_rot,
            obs_sceneitem_get_alignment,
        )
    };
    assert_eq!(
        mismatch, 0,
        "rotation={bits:#x}, alignment={alignment:#x}, flags={flags}, mismatch={mismatch}"
    );
}

#[test]
fn characterized_edges() {
    for flags in 0..2 {
        for alignment in [0, 1, 2, 4, 8, 16, 0x8000_0000, u32::MAX] {
            for bits in [
                0,
                0x8000_0000,
                0x3f80_0000,
                0xbf80_0000,
                0x7f80_0000,
                0xff80_0000,
                0x7fc1_2345,
                0xffc5_4321,
                0x7fa0_4321,
                1,
                0x7f7f_ffff,
            ] {
                check(bits, alignment, flags);
            }
        }
    }
}

#[test]
fn parent_video_mutex_blocks_and_publishes() {
    // SAFETY: the C fixture joins every worker before freeing C-owned objects.
    assert_eq!(
        unsafe {
            oracle_scene_item_transform_concurrent(
                obs_sceneitem_get_rot,
                obs_sceneitem_get_alignment,
            )
        },
        0
    );
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(1024), failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn arbitrary_fields(bits in any::<u32>(), alignment in any::<u32>(), flags in any::<u32>()) {
        check(bits, alignment, flags);
    }
}
