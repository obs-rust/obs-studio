//! Safe registration behavior and lock discipline (no independent threads).
use obs_core::tick_callbacks::{self, Registry};

#[derive(Default)]
struct List { rows: Vec<(u8, u8)>, locked: bool, locks: usize }
impl Registry for List {
    type Entry = (u8, u8);
    fn lock(&mut self) { assert!(!self.locked); self.locked = true; self.locks += 1; }
    fn unlock(&mut self) { assert!(self.locked); self.locked = false; }
    fn len(&self) -> usize { assert!(self.locked); self.rows.len() }
    fn get(&self, index: usize) -> Self::Entry { assert!(self.locked); self.rows[index] }
    fn insert(&mut self, index: usize, entry: Self::Entry) { assert!(self.locked); self.rows.insert(index, entry); }
    fn erase(&mut self, index: usize) { assert!(self.locked); self.rows.remove(index); }
}

#[test]
fn registration_order_pairs_duplicates_and_absent_removal() {
    let mut list = List::default();
    for pair in [(1, 1), (2, 1), (1, 2), (1, 1)] { tick_callbacks::add(&mut list, pair); }
    assert_eq!(list.rows, [(1, 1), (1, 2), (2, 1), (1, 1)]);
    assert_eq!(list.rows.iter().rev().copied().collect::<Vec<_>>(), [(1, 1), (2, 1), (1, 2), (1, 1)]);
    tick_callbacks::remove(&mut list, (1, 1));
    assert_eq!(list.rows, [(1, 2), (2, 1), (1, 1)]);
    tick_callbacks::remove(&mut list, (2, 2));
    assert_eq!(list.rows, [(1, 2), (2, 1), (1, 1)]);
    for pair in [(1, 1), (1, 2), (2, 1), (1, 1)] { tick_callbacks::remove(&mut list, pair); }
    assert!(list.rows.is_empty());
    assert!(!list.locked);
    assert_eq!(list.locks, 10);
}

#[test]
fn test_transition_recursive_destination_timing_tick_registration_lifetime() {
    // Registration and removal around timing_tick in the unchanged C test.
    // Frame timing itself remains covered by that live Tier 2 test.
    let mut list = List::default();
    tick_callbacks::add(&mut list, (1, 0));
    assert_eq!(list.rows, [(1, 0)]);
    tick_callbacks::remove(&mut list, (1, 0));
    assert!(list.rows.is_empty());
    assert!(!list.locked);
    assert_eq!(list.locks, 2);
}
