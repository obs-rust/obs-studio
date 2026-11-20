//! Tier 1 mirrors test_sceneitem_get_group.c: rejection before any flag read
//! or lock, one balanced full lock around the walk, and the walk's borrowed
//! result returned as is. The walk itself stays in C; the linked cases run a
//! safe model of it, so the wrapper is seen passing its first match through.
use obs_scene::item_get_group::{Query, get_group};

/// A safe scene item: group flag, removed flag, group context, reference count.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Item {
    is_group: bool,
    removed: bool,
    context: Option<usize>,
    refs: i64,
}

struct Model {
    child_is_group: bool,
    child_parent: Option<usize>,
    child_refs: i64,
    items: Vec<Item>,
    /// `Some` replaces the model walk with a recorded fixed result.
    recorded: Option<Option<usize>>,
    locked: bool,
    flag_reads: u32,
    locks: u32,
    lookups: u32,
    unlocks: u32,
}

impl Model {
    fn new() -> Self {
        Self {
            child_is_group: false,
            child_parent: Some(1),
            child_refs: 11,
            items: (0..5)
                .map(|i| Item {
                    is_group: false,
                    removed: false,
                    context: None,
                    refs: 17 + i,
                })
                .collect(),
            recorded: None,
            locked: false,
            flag_reads: 0,
            locks: 0,
            lookups: 0,
            unlocks: 0,
        }
    }

    fn assert_balanced(&self) {
        assert!(!self.locked);
        assert_eq!(self.locks, self.lookups);
        assert_eq!(self.locks, self.unlocks);
    }
}

impl Query for Model {
    type Group = usize;
    fn is_group(&mut self) -> bool {
        assert!(!self.locked, "flag read under the lock");
        self.flag_reads += 1;
        self.child_is_group
    }
    fn lock(&mut self) {
        assert!(!self.locked);
        assert_eq!(self.locks, self.lookups);
        assert_eq!(self.locks, self.unlocks);
        self.locked = true;
        self.locks += 1;
    }
    fn lookup(&mut self) -> Option<usize> {
        assert!(self.locked);
        assert_eq!(self.locks, self.lookups + 1);
        self.lookups += 1;
        if let Some(result) = self.recorded {
            return result;
        }
        // The C walk: the first group whose scene is the child's parent. It
        // never consults the removed flag, and an absent parent matches an
        // absent context.
        self.items
            .iter()
            .position(|item| item.is_group && item.context == self.child_parent)
    }
    fn unlock(&mut self) {
        assert!(self.locked);
        assert_eq!(self.locks, self.lookups);
        assert_eq!(self.locks, self.unlocks + 1);
        self.locked = false;
        self.unlocks += 1;
    }
}

#[test]
fn rejects_before_lock() {
    let mut model = Model::new();
    // NULL scene and/or item: no handle, so nothing is read or locked.
    assert_eq!(get_group::<Model>(None), None);
    model.child_is_group = true;
    assert_eq!(get_group(Some(&mut model)), None);
    assert_eq!(model.flag_reads, 1);
    assert_eq!((model.locks, model.lookups, model.unlocks), (0, 0, 0));
    model.assert_balanced();
}

#[test]
fn balanced_lookup_returns_borrowed_pointer() {
    let mut model = Model::new();
    for i in 0..6u32 {
        let result = (i < 5).then_some(i as usize);
        model.recorded = Some(result);
        assert_eq!(get_group(Some(&mut model)), result);
        assert_eq!(model.locks, i + 1);
        assert_eq!(model.lookups, model.locks);
        assert_eq!(model.unlocks, model.locks);
        assert!(!model.locked);
        assert_eq!(model.child_refs, 11);
        for (j, item) in model.items.iter().enumerate() {
            assert_eq!(item.refs, 17 + j as i64);
        }
    }
    model.assert_balanced();
}

fn linked_query(model: &mut Model, expected: Option<usize>) {
    let items = model.items.clone();
    let child = (model.child_is_group, model.child_parent, model.child_refs);
    for _ in 0..2 {
        assert_eq!(get_group(Some(&mut *model)), expected);
        // Includes every reference count: the result is borrowed.
        assert_eq!(model.items, items);
        assert_eq!(
            (model.child_is_group, model.child_parent, model.child_refs),
            child
        );
    }
    model.assert_balanced();
}

#[test]
fn linked_first_matching_parent_group() {
    let mut model = Model::new();
    let items = core::mem::take(&mut model.items);
    linked_query(&mut model, None); // empty scene
    model.items = items;
    linked_query(&mut model, None); // non-groups, even with a matching parent
    model.items[0].context = model.child_parent;
    for i in 1..4 {
        model.items[i].is_group = true;
    }
    model.items[1].context = Some(2);
    model.items[2].context = model.child_parent;
    model.items[3].context = model.child_parent;
    linked_query(&mut model, Some(2)); // skip ordinary item and foreign group
    model.items[2].removed = true;
    linked_query(&mut model, Some(2)); // characterized: removed is not consulted
    model.items[2].context = Some(2);
    linked_query(&mut model, Some(3));
    model.child_parent = Some(0);
    linked_query(&mut model, None); // no matching group / foreign child
    model.child_parent = None;
    linked_query(&mut model, None);
    model.items[1].context = None;
    linked_query(&mut model, Some(1)); // characterized: NULL parent matches NULL context
}
