//! Tier 1: `obs_scene::transform_update`, the transform update decisions of
//! `scene_video_render()` and `update_transforms_and_prune_sources()`.
//!
//! The `steps_*` cases port `test/cmocka/test_scene_transform_update_steps.c`
//! 1:1: its 9 cases under the same names, in the same order, with the same
//! inputs and expectations.
//!
//! The `edge_*` cases are new: a long run of renders against a model that
//! keeps its own history, and proptests of both decisions over any `u32`
//! sizes and flags, against models written another way.

use obs_scene::transform_update::{Size, item_needs_update, scene_size};
use proptest::prelude::*;

const MAX32: u32 = u32::MAX;

/// One scene size step: the stored size and whether it changed.
fn check_size_step(last: (u32, u32), now: (u32, u32), changed: bool) {
    let mut stored = Size::new(last.0, last.1);
    let now = Size::new(now.0, now.1);
    assert_eq!(scene_size(&mut stored, now), changed, "{last:?} -> {now:?}");
    if changed {
        assert_eq!(stored, now);
    } else {
        assert_eq!(stored, Size::new(last.0, last.1));
    }
}

fn item(update: bool, last: (u32, u32), source: (u32, u32), scene: bool) -> bool {
    item_needs_update(
        update,
        Size::new(last.0, last.1),
        Size::new(source.0, source.1),
        scene,
    )
}

#[test]
fn steps_scene_size_same() {
    for size in [(1920, 1080), (0, 0), (1, 1), (MAX32, MAX32), (MAX32, 0)] {
        check_size_step(size, size, false);
    }
}

#[test]
fn steps_scene_size_new() {
    for (last, now) in [
        // the first render: a new scene stores 0 x 0
        ((0, 0), (1920, 1080)),
        // the width alone, the height alone, both
        ((1920, 1080), (1280, 1080)),
        ((1920, 1080), (1920, 720)),
        ((1920, 1080), (1280, 720)),
        // width and height swapped
        ((1920, 1080), (1080, 1920)),
        // back to 0 x 0 (an empty scene, a canvas without video)
        ((1920, 1080), (0, 0)),
        // the limits
        ((0, 0), (MAX32, MAX32)),
        ((MAX32, MAX32), (MAX32 - 1, MAX32)),
        ((MAX32, MAX32), (MAX32, MAX32 - 1)),
    ] {
        check_size_step(last, now, true);
    }
}

#[test]
fn steps_scene_size_sequence() {
    let renders = [
        (1920, 1080, true),
        (1920, 1080, false),
        (1280, 720, true),
        (1280, 720, false),
        (1920, 1080, true),
        (1920, 1080, false),
        (1920, 720, true),
        (1920, 1080, true),
        (0, 0, true),
        (0, 0, false),
        (1920, 1080, true),
    ];
    let mut stored = Size::default();
    for (width, height, changed) in renders {
        let now = Size::new(width, height);
        assert_eq!(scene_size(&mut stored, now), changed, "{now:?}");
        assert_eq!(stored, now);
    }
}

#[test]
fn steps_item_no_update() {
    assert!(!item(false, (1920, 1080), (1920, 1080), false));
    assert!(!item(false, (0, 0), (0, 0), false));
    assert!(!item(false, (MAX32, MAX32), (MAX32, MAX32), false));
}

#[test]
fn steps_item_each_input() {
    // update_transform
    assert!(item(true, (1920, 1080), (1920, 1080), false));
    // a new source width, a new source height, both
    assert!(item(false, (1920, 1080), (1280, 1080), false));
    assert!(item(false, (1920, 1080), (1920, 720), false));
    assert!(item(false, (1920, 1080), (1280, 720), false));
    // the scene size
    assert!(item(false, (1920, 1080), (1920, 1080), true));
}

#[test]
fn steps_item_all_patterns() {
    for pattern in 0..8_u8 {
        let update = pattern & 1 != 0;
        let size_new = pattern & 2 != 0;
        let scene = pattern & 4 != 0;
        let width = if size_new { 641 } else { 640 };
        assert_eq!(
            item(update, (640, 480), (width, 480), scene),
            update || size_new || scene,
            "pattern {pattern}"
        );
    }
}

#[test]
fn steps_item_flag_ignores_size() {
    assert!(item(true, (1920, 1080), (0, 0), false));
    assert!(item(true, (0, 0), (0, 0), false));
    assert!(item(true, (MAX32, MAX32), (0, 0), true));
}

#[test]
fn steps_item_zero_sizes() {
    assert!(!item(false, (0, 0), (0, 0), false));
    assert!(item(false, (0, 0), (1, 0), false));
    assert!(item(false, (0, 0), (0, 1), false));
    assert!(item(false, (1, 1), (0, 0), false));
}

#[test]
fn steps_item_size_compare() {
    assert!(item(false, (1920, 1080), (1080, 1920), false));
    assert!(item(false, (2, 8), (4, 4), false));
    assert!(item(false, (MAX32, 0), (0, MAX32), false));
    assert!(item(false, (MAX32, MAX32), (MAX32 - 1, MAX32), false));
    assert!(item(false, (MAX32, MAX32), (MAX32, MAX32 - 1), false));
}

/// A run of renders over a few sizes, against a model that remembers the
/// size of the render before: a render is a change when its size is not
/// that one (the first render compares with 0 x 0).
#[test]
fn edge_scene_size_run() {
    let sizes = [
        Size::new(0, 0),
        Size::new(1920, 1080),
        Size::new(1080, 1920),
        Size::new(MAX32, 0),
        Size::new(0, MAX32),
    ];
    let mut stored = Size::default();
    let mut before = Size::default();
    for step in 0..500_usize {
        let now = sizes[(step * 7 + step / 3) % sizes.len()];
        assert_eq!(scene_size(&mut stored, now), before != now, "step {step}");
        assert_eq!(stored, now);
        before = now;
    }
}

fn size() -> impl Strategy<Value = Size> {
    let dim = prop_oneof![
        Just(0_u32),
        Just(1),
        Just(MAX32 - 1),
        Just(MAX32),
        any::<u32>()
    ];
    (dim.clone(), dim).prop_map(|(width, height)| Size::new(width, height))
}

proptest! {
    /// The stored size is `now` afterwards whatever happened, and the
    /// result is whether either dimension differed.
    #[test]
    fn edge_scene_size_any(last in size(), now in size()) {
        let mut stored = last;
        let changed = scene_size(&mut stored, now);
        prop_assert_eq!(stored, now);
        prop_assert_eq!(changed, (last.width, last.height) != (now.width, now.height));
    }

    /// Asking twice in a row: the second is never a change.
    #[test]
    fn edge_scene_size_idempotent(last in size(), now in size()) {
        let mut stored = last;
        let _ = scene_size(&mut stored, now);
        prop_assert!(!scene_size(&mut stored, now));
        prop_assert_eq!(stored, now);
    }

    /// The item decision against a model written as "nothing asks".
    #[test]
    fn edge_item_any(
        update in any::<bool>(),
        last in size(),
        source in size(),
        scene in any::<bool>(),
    ) {
        let nothing_asks = !update && last == source && !scene;
        prop_assert_eq!(item_needs_update(update, last, source, scene), !nothing_asks);
    }

    /// A set flag decides alone, whatever size the caller passes.
    #[test]
    fn edge_item_flag_alone(last in size(), source in size(), scene in any::<bool>()) {
        prop_assert!(item_needs_update(true, last, source, scene));
    }
}
