use obs_hotkey::unregister::{Registry, release_registerer, unregister_hotkey};

#[derive(Default)]
struct Store {
    category: i32,
    present: bool,
    owner: bool,
    freed: bool,
    events: Vec<i32>,
}

impl Registry for Store {
    type Key = ();
    fn next_id(&self) -> usize { 1 }
    fn find(&mut self, _: usize) -> Option<()> { self.present.then_some(()) }
    fn remove(&mut self, _: ()) { self.present = false; self.events.push(1); }
    fn notify(&mut self, _: ()) {
        assert!(!self.present);
        assert!(!self.freed);
        assert_eq!(self.owner, self.category != 0);
        self.events.push(2);
    }
    fn category(&self, _: ()) -> i32 { self.category }
    fn release(&mut self, _: (), category: i32) {
        self.events.push(10 + category * 2 + i32::from(self.owner));
    }
    fn clear_owner(&mut self, _: ()) { self.owner = false; self.events.push(3); }
    fn remove_bindings(&mut self, _: usize) {
        assert!(!self.owner);
        assert!(!self.freed);
        self.events.push(4);
    }
    fn free(&mut self, _: ()) { self.freed = true; self.events.push(5); }
}

#[test]
fn test_hotkey_unregister_frontend_and_owners() {
    for category in 0..5 {
        let mut store = Store { category, present: true, owner: category != 0, ..Store::default() };
        unregister_hotkey(&mut store, usize::MAX);
        unregister_hotkey(&mut store, 1000);
        assert!(store.events.is_empty());
        unregister_hotkey(&mut store, 0);
        let mut expected = vec![1, 2];
        if category != 0 { expected.push(11 + category * 2); }
        expected.push(3);
        // Characterized, not endorsed: source releases its now-null owner again.
        if category == 1 { expected.push(12); }
        expected.extend([4, 5]);
        assert_eq!(store.events, expected);
        assert!(!store.present);
        assert!(store.freed);
        unregister_hotkey(&mut store, 0);
        assert_eq!(store.events, expected);
    }
}

#[test]
fn release_registerer_clears_every_category() {
    for category in -1..7 {
        let mut store = Store { category, owner: true, ..Store::default() };
        release_registerer(&mut store, ());
        assert!(!store.owner);
        let mut expected = Vec::new();
        if (1..=4).contains(&category) { expected.push(11 + category * 2); }
        expected.push(3);
        assert_eq!(store.events, expected);
    }
}
