//! Tier 3: the verbatim C bounds setters against the Rust shims, over whole
//! real items, the same item fields lock, and recording stand-ins for the two
//! scene calls they make (the relative size conversion and the transform
//! update), each logged with whether the parent's video mutex was held.
//!
//! One intentional difference, so one input is left out on the C side:
//! `obs_sceneitem_set_bounds` with an item and a NULL `bounds` read through
//! the pointer under the item's lock and crashed. The port logs the NULL
//! parameter at LOG_DEBUG, as `obs_ptr_valid` does in the crop setter, and
//! returns before the lock (`null_bounds_is_the_intentional_difference`).
use core::mem::{offset_of, size_of};
use obs_c_oracle as _;
use obs_graphics::ffi::vec2::vec2;
use obs_scene::ffi::item_bounds_getters::SceneItemPrefix;
use obs_scene::ffi::item_bounds_setters::{
    obs_scene_bounds_valid_type, obs_sceneitem_set_bounds, obs_sceneitem_set_bounds_alignment,
    obs_sceneitem_set_bounds_crop, obs_sceneitem_set_bounds_type,
};
use proptest::prelude::*;

type SizeSetter = unsafe extern "C" fn(*mut SceneItemPrefix, *const vec2);

/// `struct bounds_setters` of the oracle.
#[repr(C)]
struct Setters {
    set_type: unsafe extern "C" fn(*mut SceneItemPrefix, u32),
    set_align: unsafe extern "C" fn(*mut SceneItemPrefix, u32),
    set_crop: unsafe extern "C" fn(*mut SceneItemPrefix, bool),
    set_size: SizeSetter,
}

const RUST: Setters = Setters {
    set_type: obs_sceneitem_set_bounds_type,
    set_align: obs_sceneitem_set_bounds_alignment,
    set_crop: obs_sceneitem_set_bounds_crop,
    set_size: obs_sceneitem_set_bounds,
};

unsafe extern "C" {
    fn oracle_scene_bounds_valid_type(value: i64) -> u32;
    fn oracle_scene_bounds_setters_layout(field: u32) -> usize;
    fn oracle_scene_bounds_setters_probe(
        which: u32,
        value: u32,
        size: *const f32,
        flags: u32,
        rust: *const Setters,
    ) -> u32;
    fn oracle_scene_bounds_setters_null(rust: *const Setters) -> u32;
    fn oracle_scene_bounds_setters_null_bounds(setter: SizeSetter) -> u32;
    fn oracle_scene_bounds_setters_concurrent(which: u32, rust: *const Setters) -> u32;
}

#[test]
fn real_header_layout() {
    let layout = [
        offset_of!(SceneItemPrefix, absolute_coordinates),
        offset_of!(SceneItemPrefix, bounds_type),
        offset_of!(SceneItemPrefix, bounds_align),
        offset_of!(SceneItemPrefix, bounds),
        offset_of!(SceneItemPrefix, crop_to_bounds),
        size_of::<vec2>(),
    ];
    for (i, value) in layout.into_iter().enumerate() {
        // SAFETY: accepts any integer and returns a real-header measurement.
        let c = unsafe { oracle_scene_bounds_setters_layout(i as u32) };
        assert_eq!(value, c, "layout {i}");
    }
}

fn check_type(value: i64) {
    // SAFETY: a pure function of its integer argument.
    let c = unsafe { oracle_scene_bounds_valid_type(value) };
    assert_eq!(obs_scene_bounds_valid_type(value), c, "{value}");
}

#[test]
fn bounds_type_check_edges() {
    for value in -16..=16 {
        check_type(value);
    }
    for value in [
        i64::MIN,
        i64::MIN + 1,
        i64::from(i32::MIN),
        i64::from(i32::MAX),
        i64::from(u32::MAX),
        1 << 32,
        (1 << 32) + 3,
        i64::MAX,
    ] {
        check_type(value);
    }
}

/// `which`: 0 type, 1 alignment, 2 crop, 3 size; `flags` as the oracle's
/// probe: a parent, absolute coordinates, the lock already held, the size
/// aliasing the item's own bounds.
fn check(which: u32, value: u32, size: [f32; 2], flags: u32) {
    // SAFETY: C owns complete live items and their parent, and calls both
    // setters under the original lifecycle and lock contract.
    let result =
        unsafe { oracle_scene_bounds_setters_probe(which, value, size.as_ptr(), flags, &RUST) };
    assert_eq!(
        result, 0,
        "which={which}, value={value:#x}, size={size:?}, flags={flags:#b}"
    );
}

const VALUES: [u32; 10] = [0, 1, 4, 6, 7, 8, 0xff, 0x7fff_ffff, 0x8000_0000, u32::MAX];
const SIZES: [[f32; 2]; 7] = [
    [0.0, 0.0],
    [1920.0, 1080.0],
    [-20.0, 35.5],
    [-0.0, f32::MIN_POSITIVE],
    [f32::MAX, f32::MIN],
    [f32::INFINITY, f32::NEG_INFINITY],
    [f32::NAN, 1.0],
];

#[test]
fn characterized_edges() {
    for flags in 0..16 {
        for which in 0..3 {
            for value in VALUES {
                check(which, value, [0.0, 0.0], flags);
            }
        }
        for size in SIZES {
            check(3, 0, size, flags);
        }
    }
}

fn size() -> impl Strategy<Value = [f32; 2]> {
    prop_oneof![
        any::<[u32; 2]>().prop_map(|bits| bits.map(f32::from_bits)),
        proptest::array::uniform2(-4096.0f32..4096.0),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn arbitrary_values(which in 0..4u32, value in any::<u32>(), size in size(), flags in 0..16u32) {
        check(which, value, size, flags);
    }

    #[test]
    fn arbitrary_bounds_types(value in any::<i64>()) {
        check_type(value);
    }
}

#[test]
fn parent_video_mutex_blocks_then_updates_after_store() {
    for which in 0..4 {
        // SAFETY: C joins each worker before freeing its complete C-owned state.
        let result = unsafe { oracle_scene_bounds_setters_concurrent(which, &RUST) };
        assert_eq!(result, 0, "which={which}");
    }
}

/// Runs `body` alone in a child process: the oracle swaps the global log
/// handler, which other tests in this binary would otherwise see.
fn in_child(name: &str, body: impl FnOnce()) {
    const CHILD: &str = "OBS_SCENE_BOUNDS_SETTERS_LOG_CHILD";
    if std::env::var_os(CHILD).is_some() {
        body();
        return;
    }
    let module = module_path!().split_once("::").unwrap().1;
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .env(CHILD, "1")
        .args(["--exact", &format!("{module}::{name}"), "--nocapture"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
}

#[test]
fn null_item_does_nothing() {
    in_child("null_item_does_nothing", || {
        // SAFETY: every setter is called with a NULL item, which both accept.
        assert_eq!(unsafe { oracle_scene_bounds_setters_null(&RUST) }, 0);
    });
}

#[test]
fn null_bounds_is_the_intentional_difference() {
    in_child("null_bounds_is_the_intentional_difference", || {
        // SAFETY: the port checks the pointer before any read; the C is not
        // called with it (it read through it).
        assert_eq!(
            unsafe { oracle_scene_bounds_setters_null_bounds(obs_sceneitem_set_bounds) },
            0
        );
    });
}
