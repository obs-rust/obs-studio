//! Bit-exact outputs, untouched info padding and complete item preservation;
//! no intended differences.
use core::mem::{align_of, offset_of, size_of};
use obs_c_oracle as _;
use obs_graphics::ffi::vec2::vec2;
use obs_scene::ffi::{
    item_bounds_getters::SceneItemPrefix,
    item_info_getters::{obs_sceneitem_get_bounds, obs_sceneitem_get_info2, obs_sceneitem_get_pos},
    transform_states::TransformInfo,
};
use proptest::prelude::*;

type Vec2Getter = unsafe extern "C" fn(*const SceneItemPrefix, *mut vec2);
type InfoGetter = unsafe extern "C" fn(*const SceneItemPrefix, *mut TransformInfo);
unsafe extern "C" {
    fn oracle_scene_item_info_layout(field: u32) -> usize;
    fn oracle_scene_item_info_probe(
        bits: *const u32,
        flags: u32,
        get_pos: Vec2Getter,
        get_bounds: Vec2Getter,
        get_info: InfoGetter,
    ) -> u32;
    fn oracle_scene_item_info_concurrent(
        get_pos: Vec2Getter,
        get_bounds: Vec2Getter,
        get_info: InfoGetter,
    ) -> u32;
}

#[test]
fn real_header_layout() {
    let values = [
        size_of::<TransformInfo>(),
        align_of::<TransformInfo>(),
        offset_of!(TransformInfo, pos),
        offset_of!(TransformInfo, rot),
        offset_of!(TransformInfo, scale),
        offset_of!(TransformInfo, alignment),
        offset_of!(TransformInfo, bounds_type),
        offset_of!(TransformInfo, bounds_alignment),
        offset_of!(TransformInfo, bounds),
        offset_of!(TransformInfo, crop_to_bounds),
        size_of::<u32>(),
        offset_of!(SceneItemPrefix, absolute_coordinates),
        offset_of!(SceneItemPrefix, pos),
        offset_of!(SceneItemPrefix, rot),
        offset_of!(SceneItemPrefix, align),
        offset_of!(SceneItemPrefix, bounds_type),
        offset_of!(SceneItemPrefix, bounds_align),
        offset_of!(SceneItemPrefix, bounds),
        offset_of!(SceneItemPrefix, crop_to_bounds),
    ];
    // vec2 and the complete reused item prefix are measured field by field
    // in item_scale_getter_parity and item_bounds_getters_parity.
    for (index, value) in values.into_iter().enumerate() {
        // SAFETY: the oracle only returns layout constants from the real headers.
        assert_eq!(
            value,
            unsafe { oracle_scene_item_info_layout(index as u32) },
            "field {index}"
        );
    }
}

fn check(bits: [u32; 11], flags: u32) {
    // SAFETY: C owns complete live items, mutexes and guarded output storage,
    // and invokes the shims only during those objects' lifetimes.
    assert_eq!(
        unsafe {
            oracle_scene_item_info_probe(
                bits.as_ptr(),
                flags,
                obs_sceneitem_get_pos,
                obs_sceneitem_get_bounds,
                obs_sceneitem_get_info2,
            )
        },
        0,
        "bits={bits:08x?} flags={flags}"
    );
}

const EDGES: [u32; 11] = [
    0, 0x80000000, 0x3f800000, 0xbf800000, 0x7f800000, 0xff800000, 0x7fc01234, 0xffc05678,
    0x7f801234, 1, 0x7f7fffff,
];

#[test]
fn characterized_edges() {
    for flags in 0..64 {
        for index in 0..EDGES.len() {
            let mut bits = [0; 11];
            for (offset, value) in bits.iter_mut().enumerate() {
                *value = EDGES[(index + offset) % EDGES.len()];
            }
            for reference in [0.0f32, -0.0, 540.0, 1080.0, f32::INFINITY, f32::NAN] {
                bits[10] = reference.to_bits();
                check(bits, flags);
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
    fn arbitrary_bits(bits: [u32; 11], flags in 0u32..64) {
        check(bits, flags);
    }
}

#[test]
fn parent_video_mutex_blocks_and_publishes() {
    // SAFETY: the C fixture owns and joins each thread before freeing its items,
    // parent mutex and events. Twelve threads run sequentially, one at a time.
    assert_eq!(
        unsafe {
            oracle_scene_item_info_concurrent(
                obs_sceneitem_get_pos,
                obs_sceneitem_get_bounds,
                obs_sceneitem_get_info2,
            )
        },
        0
    );
}
