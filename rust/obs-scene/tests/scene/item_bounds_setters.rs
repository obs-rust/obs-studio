//! Tier 1 mirrors test_scene_item_bounds_setters.c: the stored values, the
//! bounds type check, the relative or absolute size store, and the order of
//! lock, store, unlock and update. The update itself stays C
//! (`do_update_transform`); the model item follows it, so each case keeps
//! the C test's checks of `update_transform` and `defer_update`. A NULL
//! bounds pointer is the intentional difference: it stores nothing and
//! takes no lock.
use obs_graphics::vec2::Vec2;
use obs_scene::coords::size_from_absolute;
use obs_scene::item_bounds_setters::{
    BOUNDS_MAX_ONLY, BOUNDS_NONE, Fields, Skip, Store, check, set, store, valid_bounds_type,
};

/// The item's parent, as `do_update_transform` sees it.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum Parent {
    #[default]
    None,
    Scene,
    Group,
}

/// A model item: the fields the setters write and the calls they make.
#[derive(Default)]
struct Item {
    calls: Vec<&'static str>,
    kind: u32,
    alignment: u32,
    crop: bool,
    size: [f32; 2],
    absolute: bool,
    parent: Parent,
    update_transform: bool,
    defer_update: i64,
    transforms: u32,
}

impl Fields for Item {
    fn lock(&mut self) {
        self.calls.push("lock");
    }
    fn unlock(&mut self) {
        self.calls.push("unlock");
    }
    /// `do_update_transform`: no parent or a group marks the item; a scene
    /// updates it now, unless `update_item_transform` is deferred.
    fn update(&mut self) {
        self.calls.push("update");
        match self.parent {
            Parent::None | Parent::Group => self.update_transform = true,
            Parent::Scene if self.defer_update > 0 => {}
            Parent::Scene => self.transforms += 1,
        }
    }
}

fn set_kind(item: &mut Item, kind: i64) {
    set(item, |item| {
        item.calls.push("write");
        item.kind = valid_bounds_type(kind);
    });
}

fn set_size(item: &mut Item, size: [f32; 2], canvas_y: f32) {
    set(item, |item| {
        item.calls.push("write");
        let size = Vec2::new(size[0], size[1]);
        let stored = match store(item.absolute) {
            Store::Convert => size_from_absolute(size, canvas_y),
            Store::Copy => size,
        };
        item.size = [stored.x, stored.y];
    });
}

/// The calls since the last check, and whether the item was marked; the
/// mark is cleared for the next setter, as the C test does.
fn calls(item: &mut Item) -> Vec<&'static str> {
    assert!(item.update_transform, "update_transform not set");
    item.update_transform = false;
    std::mem::take(&mut item.calls)
}

const ORDER: [&str; 4] = ["lock", "write", "unlock", "update"];

#[test]
fn every_bounds_type_in_the_enum_is_kept() {
    for kind in BOUNDS_NONE..=BOUNDS_MAX_ONLY {
        assert_eq!(valid_bounds_type(i64::from(kind)), kind);
    }
    assert_eq!(BOUNDS_NONE, 0);
    assert_eq!(BOUNDS_MAX_ONLY, 6);
}

#[test]
fn a_bounds_type_outside_the_enum_is_none() {
    for kind in [
        -1,
        7,
        i64::from(i32::MAX),
        i64::from(i32::MIN),
        i64::from(u32::MAX),
        1 << 32,
        (1 << 32) + 1,
        i64::MIN,
        i64::MAX,
    ] {
        assert_eq!(valid_bounds_type(kind), BOUNDS_NONE, "{kind}");
    }
}

/// `bounds_writes`: no parent, then a group parent, absolute coordinates.
#[test]
fn every_setter_stores_under_the_lock_then_updates() {
    for parent in [Parent::None, Parent::Group] {
        let mut item = Item {
            parent,
            absolute: true,
            ..Item::default()
        };
        for kind in BOUNDS_NONE..=BOUNDS_MAX_ONLY {
            set_kind(&mut item, i64::from(kind));
            assert_eq!(calls(&mut item), ORDER);
            assert_eq!(item.kind, kind);
        }
        for kind in [-1, 7, i64::from(i32::MAX)] {
            item.kind = 1;
            set_kind(&mut item, kind);
            assert_eq!(calls(&mut item), ORDER);
            assert_eq!(item.kind, BOUNDS_NONE);
        }
        // OBS_ALIGN_LEFT | OBS_ALIGN_TOP; unknown bits are kept.
        for alignment in [0, 1 | 4, u32::MAX] {
            set(&mut item, |item| {
                item.calls.push("write");
                item.alignment = alignment;
            });
            assert_eq!(calls(&mut item), ORDER);
            assert_eq!(item.alignment, alignment);
        }
        for crop in [false, true] {
            set(&mut item, |item| {
                item.calls.push("write");
                item.crop = crop;
            });
            assert_eq!(calls(&mut item), ORDER);
            assert_eq!(item.crop, crop);
        }
        for size in [[0.0, 0.0], [1920.0, 1080.0], [-20.0, 35.5]] {
            set_size(&mut item, size, 1080.0);
            assert_eq!(calls(&mut item), ORDER);
            assert_eq!(item.size, size);
        }
        assert_eq!(item.transforms, 0);
    }
}

#[test]
fn absolute_bounds_are_copied() {
    assert_eq!(store(true), Store::Copy);
}

/// `relative_bounds`: a 1920x1080 scene parent with a deferred update.
#[test]
fn relative_bounds_are_converted() {
    assert_eq!(store(false), Store::Convert);
    let mut item = Item {
        parent: Parent::Scene,
        defer_update: 1,
        ..Item::default()
    };
    let input = [540.0, -270.0];
    set_size(&mut item, input, 1080.0);
    assert_eq!(item.calls, ORDER);
    assert_eq!(item.size, [1.0, -0.5]);
    // The scene asks for an immediate update, which the defer suppresses.
    assert!(!item.update_transform);
    assert_eq!(item.transforms, 0);
    assert_eq!(item.defer_update, 1);
    assert_eq!(input, [540.0, -270.0]);
}

#[test]
fn a_null_item_is_skipped_before_a_null_bounds() {
    assert_eq!(check(true, true), Ok(()));
    assert_eq!(check(false, true), Err(Skip::Item));
    assert_eq!(check(false, false), Err(Skip::Item));
}

/// The intentional difference: the C read through a NULL bounds pointer
/// under the item's lock. The port stops first, before the lock.
#[test]
fn a_null_bounds_is_skipped() {
    assert_eq!(check(true, false), Err(Skip::Bounds));
}
