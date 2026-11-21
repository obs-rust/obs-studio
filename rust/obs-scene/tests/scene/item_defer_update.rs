//! Tier 1 mirrors test_scene_item_defer_update.c. The downstream transform
//! adapter is modeled as in item_bounds_setters; actual dispatch stays Tier 2.
use obs_scene::item_defer_update::{self as defer, TransformDeferral};
use std::cell::{Cell, RefCell};

#[derive(Clone, Copy, Default)]
enum Parent {
    #[default]
    Scene,
    Group,
    None,
}

#[derive(Default)]
struct Item {
    count: Cell<isize>,
    parent: Parent,
    pending: Cell<bool>,
    transforms: Cell<usize>,
    calls: RefCell<Vec<&'static str>>,
}

impl TransformDeferral for Item {
    fn increment(&self) {
        self.calls.borrow_mut().push("increment");
        self.count.set(self.count.get() + 1);
    }
    fn decrement(&self) -> isize {
        self.calls.borrow_mut().push("decrement");
        let count = self.count.get() - 1;
        self.count.set(count);
        count
    }
    fn update_transform(&self) {
        self.calls.borrow_mut().push("update");
        assert_eq!(self.count.get(), 0, "decrement must finish before dispatch");
        match self.parent {
            Parent::Scene => self.transforms.set(self.transforms.get() + 1),
            Parent::Group | Parent::None => self.pending.set(true),
        }
    }
}

#[test]
fn plain_pair_computes_on_end() {
    let item = Item::default();
    for expected in 1..=3 {
        defer::begin(&item);
        assert_eq!(item.count.get(), 1);
        assert_eq!(item.transforms.get(), expected - 1);
        defer::end(&item);
        assert_eq!(item.count.get(), 0);
        assert_eq!(item.transforms.get(), expected);
    }
    assert_eq!(
        *item.calls.borrow(),
        ["increment", "decrement", "update"].repeat(3)
    );
}

#[test]
fn nested_pairs_compute_on_outer_end() {
    let item = Item::default();
    for _ in 0..3 {
        defer::begin(&item);
    }
    for expected in [2, 1] {
        defer::end(&item);
        assert_eq!(item.count.get(), expected);
        assert_eq!(item.transforms.get(), 0);
    }
    defer::end(&item);
    assert_eq!(item.transforms.get(), 1);
}

#[test]
fn unmatched_end_goes_negative_and_begin_reaching_zero_is_quiet() {
    let item = Item::default();
    for expected in [-1, -2] {
        defer::end(&item);
        assert_eq!(item.count.get(), expected);
        assert_eq!(item.transforms.get(), 0);
    }
    for expected in [-1, 0] {
        defer::begin(&item);
        assert_eq!(item.count.get(), expected);
        assert_eq!(item.transforms.get(), 0);
    }
    defer::begin(&item);
    defer::end(&item);
    assert_eq!(item.transforms.get(), 1);
}

#[test]
fn group_child_end_sets_flag_and_parentless_end_reads_nothing() {
    for parent in [Parent::Group, Parent::None] {
        let item = Item {
            parent,
            ..Item::default()
        };
        defer::begin(&item);
        defer::begin(&item);
        defer::end(&item);
        assert!(!item.pending.get());
        defer::end(&item);
        assert!(item.pending.get());
        assert_eq!(item.transforms.get(), 0);
        assert_eq!(
            *item.calls.borrow(),
            ["increment", "increment", "decrement", "decrement", "update"]
        );
    }
}

#[test]
fn group_item_end_computes_like_an_ordinary_item() {
    let item = Item::default();
    defer::begin(&item);
    assert_eq!(item.transforms.get(), 0);
    defer::end(&item);
    assert_eq!(item.transforms.get(), 1);
}

#[test]
fn dispatch_uses_the_decrement_result_without_reloading() {
    struct RacingItem(Cell<isize>, Cell<bool>);
    impl TransformDeferral for RacingItem {
        fn increment(&self) {
            self.0.set(self.0.get() + 1);
        }
        fn decrement(&self) -> isize {
            let result = self.0.get() - 1;
            self.0.set(77); // another caller can increment after this RMW
            result
        }
        fn update_transform(&self) {
            self.1.set(true);
        }
    }
    let item = RacingItem(Cell::new(1), Cell::new(false));
    defer::end(&item);
    assert!(item.1.get());
    assert_eq!(item.0.get(), 77);
}
