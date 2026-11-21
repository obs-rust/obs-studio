//! Tier 1 mirrors test_sceneitem_order_position.c: NULL and detached items
//! are 0 before any lock; otherwise the zero-based sibling position (or the
//! list length for an item its parent list does not hold) under exactly one
//! balanced full lock, with the head read before it. The list is a safe model
//! of the C-owned scene and item links.
use obs_scene::item_order::item_attached;
use obs_scene::item_order_position::{ParentList, order_position};

/// A safe scene item: its link, and fields that must not affect position.
#[derive(Clone, Copy, Default)]
struct Item {
    next: Option<usize>,
    id: i64,
    user_visible: bool,
    is_group: bool,
}

/// One parent scene's list over a shared item arena.
struct Scene<'a> {
    first: Option<usize>,
    items: &'a [Item],
    target: usize,
    locked: bool,
    heads: u32,
    locks: u32,
    unlocks: u32,
}

impl<'a> Scene<'a> {
    fn new(first: Option<usize>, items: &'a [Item], target: usize) -> Self {
        Self {
            first,
            items,
            target,
            locked: false,
            heads: 0,
            locks: 0,
            unlocks: 0,
        }
    }
}

impl ParentList for Scene<'_> {
    type Item = usize;
    fn target(&self) -> usize {
        self.target
    }
    fn head(&mut self) -> Option<usize> {
        assert!(!self.locked, "head read under the lock");
        assert_eq!(self.locks, 0, "head read after the lock");
        self.heads += 1;
        self.first
    }
    fn lock(&mut self) {
        assert!(!self.locked);
        assert_eq!(self.heads, 1, "lock before the head read");
        self.locked = true;
        self.locks += 1;
    }
    fn next(&mut self, item: usize) -> Option<usize> {
        assert!(self.locked, "walk outside the lock");
        self.items[item].next
    }
    fn unlock(&mut self) {
        assert!(self.locked);
        self.locked = false;
        self.unlocks += 1;
    }
}

/// assert_position(): one lock and one unlock, and the expected position.
fn assert_position(first: Option<usize>, items: &[Item], target: usize, expected: i32) {
    let mut scene = Scene::new(first, items, target);
    assert_eq!(order_position(Some(&mut scene)), expected);
    assert_eq!(scene.locks, 1);
    assert_eq!(scene.unlocks, 1);
    assert!(!scene.locked);
}

/// link_items(): items[0..count] in order, returning the head.
fn link_items(items: &mut [Item], count: usize) -> Option<usize> {
    for (i, item) in items.iter_mut().enumerate().take(count) {
        item.next = (i + 1 < count).then_some(i + 1);
    }
    (count != 0).then_some(0)
}

#[test]
fn null_and_detached_return_zero() {
    // The attached predicate the shim applies: NULL (no item) and detached
    // (no parent) never reach a list, so nothing is read or locked.
    assert!(!item_attached(false, false));
    assert!(!item_attached(true, false));
    assert!(item_attached(true, true));
    assert_eq!(order_position::<Scene>(None), 0);
    assert_eq!(order_position::<Scene>(None), 0);
}

#[test]
fn positions_count_siblings() {
    let mut items = [Item::default(); 6];
    for count in 1..=6 {
        let first = link_items(&mut items, count);
        for i in 0..count {
            // IDs and visibility do not affect position.
            items[i].id = 99 - i as i64;
            items[i].user_visible = i % 2 == 0;
            assert_position(first, &items, i, i as i32);
        }
    }
    // Re-link the same handles in a different order.
    let first = Some(5);
    for (i, item) in items.iter_mut().enumerate() {
        item.next = i.checked_sub(1);
    }
    for i in 0..6 {
        assert_position(first, &items, 5 - i, i as i32);
    }
}

#[test]
fn nested_groups_count_only_the_parent_list() {
    let mut roots = [Item::default(); 3];
    let mut children = [Item::default(); 4];
    let mut grandchildren = [Item::default(); 2];
    let root = link_items(&mut roots, 3);
    let group = link_items(&mut children, 4);
    let nested = link_items(&mut grandchildren, 2);
    roots[1].is_group = true;
    children[2].is_group = true;
    // A group occupies one sibling slot; its descendants never inflate it.
    // Each child's parent is the group's own scene, including nested groups.
    for i in 0..3 {
        assert_position(root, &roots, i, i as i32);
    }
    for i in 0..4 {
        assert_position(group, &children, i, i as i32);
    }
    for i in 0..2 {
        assert_position(nested, &grandchildren, i, i as i32);
    }
}

#[test]
fn absent_item_returns_list_length() {
    // items[3] is the absent item: its parent is the scene, but the list
    // holds only items[0..count]. Inconsistent membership is characterized,
    // not endorsed: count until the end of the list.
    let mut items = [Item::default(); 4];
    for count in 0..=3 {
        let first = link_items(&mut items, count);
        assert_position(first, &items, 3, count as i32);
    }
}
