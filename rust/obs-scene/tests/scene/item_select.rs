//! Mirrors test_scene_item_select.c with safe synchronous callbacks.
use obs_scene::item_select::{self, Item};
use std::cell::{Cell, RefCell};

struct Model {
    selected: Cell<bool>,
    parent: Cell<bool>,
    reenter: bool,
    events: RefCell<Vec<bool>>,
}
impl Item for Model {
    fn selected(&self) -> bool {
        self.selected.get()
    }
    fn has_parent(&self) -> bool {
        self.parent.get()
    }
    fn store(&self, value: bool) {
        self.selected.set(value);
    }
    fn signal(&self, value: bool) {
        assert_eq!(self.selected.get(), value);
        self.events.borrow_mut().push(value);
        let count = self.events.borrow().len();
        item_select::select(Some(self), value);
        assert_eq!(self.events.borrow().len(), count);
        if value && self.reenter {
            item_select::select(Some(self), false);
        }
    }
}
#[test]
fn null_item() {
    item_select::select(None::<&Model>, false);
    item_select::select(None::<&Model>, true);
}
#[test]
fn scene_group_and_reentrant_selection() {
    for _grouped in [false, true] {
        for reenter in [false, true] {
            let item = Model {
                selected: Cell::new(false),
                parent: Cell::new(true),
                reenter,
                events: RefCell::new(vec![]),
            };
            item_select::select(Some(&item), false);
            assert!(item.events.borrow().is_empty());
            item_select::select(Some(&item), true);
            assert_eq!(item.selected.get(), !reenter);
            if !reenter {
                item_select::select(Some(&item), true);
                assert_eq!(&*item.events.borrow(), &[true]);
                item_select::select(Some(&item), false);
            }
            item_select::select(Some(&item), false);
            assert_eq!(&*item.events.borrow(), &[true, false]);
            item.parent.set(false);
            item_select::select(Some(&item), true);
            item_select::select(Some(&item), false);
            assert!(!item.selected.get());
            assert_eq!(&*item.events.borrow(), &[true, false]);
        }
    }
}
