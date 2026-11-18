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
    fn next_id(&self) -> usize {
        1
    }
    fn find(&mut self, _: usize) -> Option<()> {
        self.present.then_some(())
    }
    fn remove(&mut self, _: ()) {
        self.present = false;
        self.events.push(1);
    }
    fn notify(&mut self, _: ()) {
        assert!(!self.present);
        assert!(!self.freed);
        assert_eq!(self.owner, self.category != 0);
        self.events.push(2);
    }
    fn category(&self, _: ()) -> i32 {
        self.category
    }
    fn release(&mut self, _: (), category: i32) {
        self.events.push(10 + category * 2 + i32::from(self.owner));
    }
    fn clear_owner(&mut self, _: ()) {
        self.owner = false;
        self.events.push(3);
    }
    fn remove_bindings(&mut self, _: usize) {
        assert!(!self.owner);
        assert!(!self.freed);
        self.events.push(4);
    }
    fn free_name(&mut self, _: ()) {
        self.events.push(5);
    }
    fn free_description(&mut self, _: ()) {
        self.events.push(6);
    }
    fn free(&mut self, _: ()) {
        self.freed = true;
        self.events.push(7);
    }
}

#[test]
fn test_hotkey_unregister_frontend_and_owners() {
    for category in 0..5 {
        let mut store = Store {
            category,
            present: true,
            owner: category != 0,
            ..Store::default()
        };
        unregister_hotkey(&mut store, usize::MAX);
        unregister_hotkey(&mut store, 1000);
        assert!(store.events.is_empty());
        unregister_hotkey(&mut store, 0);
        let mut expected = vec![1, 2];
        if category != 0 {
            expected.push(11 + category * 2);
        }
        expected.push(3);
        // Characterized, not endorsed: source releases its now-null owner again.
        if category == 1 {
            expected.push(12);
        }
        expected.extend([4, 5, 6, 7]);
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
        let mut store = Store {
            category,
            owner: true,
            ..Store::default()
        };
        release_registerer(&mut store, ());
        assert!(!store.owner);
        let mut expected = Vec::new();
        if (1..=4).contains(&category) {
            expected.push(11 + category * 2);
        }
        expected.push(3);
        assert_eq!(store.events, expected);
    }
}

unsafe extern "C" {
    fn oracle_hotkey_unregister_trace(
        category: i32,
        owner: bool,
        present: bool,
        id: usize,
        events: *mut i32,
    ) -> usize;
}

fn compare_removal(store: &mut Store, id: usize) {
    let mut events = [0; 16];
    // SAFETY: the oracle writes at most 16 integers into this live output array.
    // Its registry and trace are private to this thread and freed before return.
    let count = unsafe {
        oracle_hotkey_unregister_trace(
            store.category,
            store.owner,
            store.present,
            id,
            events.as_mut_ptr(),
        )
    };
    let before = store.events.len();
    unregister_hotkey(store, id);
    assert_eq!(&store.events[before..], &events[..count]);
}

#[test]
fn original_c_edges() {
    for category in 0..5 {
        for id in [0, 1, 1000, usize::MAX] {
            let mut store = Store {
                category,
                present: true,
                owner: category != 0,
                ..Store::default()
            };
            compare_removal(&mut store, id);
            compare_removal(&mut store, id);
        }
    }
}

use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn randomized_sequences_match_original_c(
        steps in prop::collection::vec((any::<bool>(), 0i32..5, prop_oneof![0usize..5, Just(usize::MAX)]), 0..64),
    ) {
        let mut store = Store::default();
        for (register, category, id) in steps {
            if register {
                store = Store { category, present: true, owner: category != 0, ..Store::default() };
            }
            compare_removal(&mut store, id);
        }
    }
}
