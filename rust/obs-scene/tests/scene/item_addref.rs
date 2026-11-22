//! Tier 1 mirrors test_sceneitem_addref.c through the safe core. The item is
//! modeled with the fields the C test fills; the real layout, the native long
//! width and the atomic stay Tier 2 and Tier 3.
use core::ffi::c_long;
use obs_scene::item_addref::{ItemRef, addref};
use std::cell::Cell;

/// The fields test_sceneitem_addref.c sets, with its neighbor links.
#[derive(Clone, Debug, PartialEq)]
struct Fields {
    refs: c_long,
    removed: bool,
    id: i64,
    active_refs: c_long,
    defer_update: c_long,
    defer_group_resize: c_long,
    visible: bool,
    selected: bool,
    locked: bool,
    crop_left: i32,
    prev: Option<usize>,
    next: Option<usize>,
}

struct Item {
    fields: Cell<Option<Fields>>,
    increments: Cell<usize>,
}

impl Item {
    fn new(fields: Fields) -> Self {
        Self {
            fields: Cell::new(Some(fields)),
            increments: Cell::new(0),
        }
    }
    fn get(&self) -> Fields {
        let fields = self.fields.take().unwrap();
        self.fields.set(Some(fields.clone()));
        fields
    }
}

impl ItemRef for Item {
    fn increment(&self) {
        let mut fields = self.fields.take().unwrap();
        fields.refs = fields.refs.wrapping_add(1);
        self.fields.set(Some(fields));
        self.increments.set(self.increments.get() + 1);
    }
}

/// Mirrors null_is_noop: the absent item is never touched.
#[test]
fn test_sceneitem_addref_null_is_noop() {
    struct Untouchable;
    impl ItemRef for Untouchable {
        fn increment(&self) {
            panic!("a NULL item must not be incremented");
        }
    }
    addref(None::<&Untouchable>);
}

/// Mirrors increments_only_item_ref: each call adds exactly one to the stored
/// count, zero and negative counts included, and changes no other field of
/// the item or its neighbor.
#[test]
fn test_sceneitem_addref_increments_only_item_ref() {
    // Stay below LONG_MAX so every addition is representable on every host.
    let counts = [c_long::MIN, -3, -1, 0, 1, 42, c_long::MAX - 3];
    for count in counts {
        let neighbor_fields = Fields {
            refs: 71,
            removed: false,
            id: 0,
            active_refs: 0,
            defer_update: 0,
            defer_group_resize: 0,
            visible: false,
            selected: false,
            locked: false,
            crop_left: 0,
            prev: None,
            next: Some(0),
        };
        let item = Item::new(Fields {
            refs: count,
            removed: true,
            id: 123_456_789,
            active_refs: 17,
            defer_update: 23,
            defer_group_resize: 29,
            visible: true,
            selected: true,
            locked: true,
            crop_left: -11,
            prev: Some(1),
            next: Some(1),
        });
        let neighbor = Item::new(neighbor_fields.clone());
        let mut expected = item.get();
        for increment in 1..=3 {
            addref(Some(&item));
            expected.refs = count + increment;
            assert_eq!(item.get().refs, expected.refs, "count {count}");
            assert_eq!(item.get(), expected, "count {count}");
            assert_eq!(neighbor.get(), neighbor_fields, "count {count}");
            assert_eq!(item.increments.get(), increment as usize);
            assert_eq!(neighbor.increments.get(), 0);
        }
    }
}

/// New edge: the core never reads the count, so a count at the native limit
/// wraps as the C atomic does (characterized, not endorsed).
#[test]
fn addref_does_not_inspect_the_count() {
    let item = Item::new(Fields {
        refs: c_long::MAX,
        removed: false,
        id: 0,
        active_refs: 0,
        defer_update: 0,
        defer_group_resize: 0,
        visible: false,
        selected: false,
        locked: false,
        crop_left: 0,
        prev: None,
        next: None,
    });
    addref(Some(&item));
    assert_eq!(item.get().refs, c_long::MIN);
    assert_eq!(item.increments.get(), 1);
}
