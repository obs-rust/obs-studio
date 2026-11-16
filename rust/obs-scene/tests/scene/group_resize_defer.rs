use obs_scene::group_resize_defer::{self, ResizeDeferral};
use std::cell::Cell;

#[derive(Default)]
struct Item {
    count: Cell<isize>,
    update: Cell<bool>,
    writes: Cell<usize>,
}
impl ResizeDeferral for Item {
    fn increment(&self) {
        self.count.set(self.count.get().wrapping_add(1));
    }
    fn decrement(&self) -> isize {
        let count = self.count.get().wrapping_sub(1);
        self.count.set(count);
        count
    }
    fn schedule_resize(&self) {
        self.update.set(true);
        self.writes.set(self.writes.get() + 1);
    }
}

#[test]
fn nested_deferral_only_schedules_on_final_end() {
    let item = Item::default();
    group_resize_defer::begin(&item);
    group_resize_defer::begin(&item);
    assert_eq!(item.count.get(), 2);
    assert!(!item.update.get());
    group_resize_defer::end(&item);
    assert_eq!(item.count.get(), 1);
    assert!(!item.update.get());
    group_resize_defer::end(&item);
    assert_eq!(item.count.get(), 0);
    assert!(item.update.get());
    assert_eq!(item.writes.get(), 1);
}

#[test]
fn nonzero_end_preserves_flag_and_unmatched_end_is_not_clamped() {
    for initial in [-2, 0, 1, 2] {
        for flag in [false, true] {
            let item = Item::default();
            item.count.set(initial);
            item.update.set(flag);
            group_resize_defer::end(&item);
            assert_eq!(item.count.get(), initial - 1);
            assert_eq!(item.update.get(), flag || initial == 1);
            assert_eq!(item.writes.get(), usize::from(initial == 1));
        }
    }
}

#[test]
fn begin_preserves_already_pending_resize() {
    let item = Item::default();
    item.update.set(true);
    group_resize_defer::begin(&item);
    assert_eq!(item.count.get(), 1);
    assert!(item.update.get());
    assert_eq!(item.writes.get(), 0);
}
