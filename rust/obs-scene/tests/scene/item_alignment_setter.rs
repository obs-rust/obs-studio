//! Tier 1 mirrors every assertion in test_sceneitem_alignment_setter.c.
use obs_scene::item_alignment_setter::{Item, set};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Parent {
    Detached,
    Scene,
    Group,
    Replacement,
}

struct Model {
    parent: Parent,
    replacement: Option<Parent>,
    captured: Option<Parent>,
    locked: bool,
    alignment: u32,
    expected: u32,
    old: u32,
    calls: Vec<&'static str>,
    dirty: bool,
    defer: i64,
    width_calls: u32,
    height_calls: u32,
    signals: u32,
    last_size: (u32, u32),
}

impl Model {
    fn new(parent: Parent, alignment: u32) -> Self {
        Self {
            parent,
            replacement: None,
            captured: None,
            locked: false,
            alignment,
            expected: alignment,
            old: alignment,
            calls: Vec::new(),
            dirty: false,
            defer: 0,
            width_calls: 0,
            height_calls: 0,
            signals: 0,
            last_size: (0, 0),
        }
    }
}

impl Item for Model {
    type Parent = Parent;
    fn lock(&mut self) -> Parent {
        assert!(self.calls.is_empty());
        assert_eq!(self.alignment, self.old);
        let parent = self.parent;
        self.captured = Some(parent);
        self.locked = parent != Parent::Detached;
        self.calls.push("lock");
        if let Some(replacement) = self.replacement {
            self.parent = replacement;
        }
        parent
    }
    fn store(&mut self, alignment: u32) {
        assert_eq!(self.calls, ["lock"]);
        assert_eq!(self.locked, self.captured != Some(Parent::Detached));
        self.alignment = alignment;
        self.calls.push("store");
    }
    fn unlock(&mut self, parent: Parent) {
        assert_eq!(Some(parent), self.captured);
        assert_eq!(self.calls, ["lock", "store"]);
        assert_eq!(self.alignment, self.expected);
        assert_eq!(self.locked, parent != Parent::Detached);
        self.locked = false;
        self.calls.push("unlock");
    }
    fn update(&mut self) {
        assert!(!self.locked);
        assert_eq!(self.calls, ["lock", "store", "unlock"]);
        assert_eq!(self.alignment, self.expected);
        self.calls.push("update");
        // C-owned dispatch model, as in the bounds setter Tier 1 tests.
        match self.parent {
            Parent::Detached | Parent::Group => self.dirty = true,
            Parent::Scene | Parent::Replacement if self.defer > 0 => {}
            Parent::Scene | Parent::Replacement => {
                self.width_calls += 1;
                self.height_calls += 1;
                self.signals += 1;
                self.last_size = (16, 8);
            }
        }
    }
}

const BITS: [u32; 5] = [0, 1 | 4, 2 | 8, 0x8000_0000, u32::MAX];
const ORDER: [&str; 4] = ["lock", "store", "unlock", "update"];

#[test]
fn test_sceneitem_alignment_setter_null_is_silent() {
    set::<Model>(None, u32::MAX);
}

#[test]
fn test_sceneitem_alignment_setter_captured_parent_and_order() {
    for parent in [Parent::Detached, Parent::Scene] {
        for bits in BITS {
            for unchanged in [false, true] {
                let old = if unchanged { bits } else { !bits };
                let mut item = Model::new(parent, old);
                item.expected = bits;
                item.replacement = Some(if parent == Parent::Scene {
                    Parent::Replacement
                } else {
                    Parent::Scene
                });
                set(Some(&mut item), bits);
                assert_eq!(item.calls, ORDER);
                assert_eq!(item.parent, item.replacement.unwrap());
                assert_eq!(item.alignment, bits);
                assert!(!item.locked);
            }
        }
    }
}

#[test]
fn test_sceneitem_alignment_setter_live_update_paths() {
    for parent in [Parent::Detached, Parent::Group, Parent::Scene] {
        for defer in [-1, 0, 1, 3, i64::from(i32::MIN), i64::from(i32::MAX)] {
            for bits in BITS {
                for dirty in [false, true] {
                    let mut item = Model::new(parent, 0);
                    item.defer = defer;
                    item.expected = bits;
                    for _ in 0..2 {
                        item.calls.clear();
                        item.old = item.alignment;
                        item.width_calls = 0;
                        item.height_calls = 0;
                        item.signals = 0;
                        item.dirty = dirty;
                        set(Some(&mut item), bits);
                        assert_eq!(item.calls, ORDER);
                        assert_eq!(item.alignment, bits);
                        assert_eq!(item.defer, defer);
                        assert_eq!(item.dirty, dirty || parent != Parent::Scene);
                        let immediate = u32::from(parent == Parent::Scene && defer <= 0);
                        assert_eq!(item.width_calls, immediate);
                        assert_eq!(item.height_calls, immediate);
                        assert_eq!(item.signals, immediate);
                        if immediate != 0 {
                            assert_eq!(item.last_size, (16, 8));
                        }
                        assert!(!item.locked);
                    }
                }
            }
        }
    }
}
