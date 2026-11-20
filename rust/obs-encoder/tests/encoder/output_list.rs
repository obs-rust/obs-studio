//! Tier 1: `obs_encoder::output_list`, the safe core of
//! `obs_encoder_add_output()` and `obs_encoder_remove_output()`.
//!
//! The `characterization_*` cases port every assertion of the unchanged
//! `test/cmocka/test_encoder_outputs.c` onto a recording list, its four cases
//! one for one: the lock sees the list as it was before the call and the
//! unlock the list as it is after, each exactly once and never nested;
//! appends keep their order and removal erases the exact output; an absent
//! output leaves the list, its buffer and its capacity as they were;
//! duplicates are accepted and only the first match is erased; and a NULL
//! encoder or output neither locks nor changes anything, not even on an
//! encoder whose mutex was never initialized. The C fixture's buffer address
//! is the recording list's reallocation count here, grown as `darray` grows.
//!
//! The `edge_*` cases are new: every read and change happens between the one
//! lock and the one unlock, an append is one push and nothing else, removal
//! erases at the first match's index and stops, and a proptest over any
//! sequence of adds and removes (NULL arguments included) against a model of
//! the C list.

use obs_encoder::output_list::{Outputs, add_output, remove_output};
use proptest::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Lock,
    Unlock,
    Push(u32),
    Count,
    At(usize),
    Erase(usize),
}

/// The C fixture's encoder: its list, `darray` capacity and buffer (counted
/// by reallocations), the mutex state and the lists the lock and unlock
/// expect to see.
#[derive(Debug)]
struct Fixture {
    initialized: bool,
    list: Vec<u32>,
    capacity: usize,
    reallocations: usize,
    locked: bool,
    lock_calls: u32,
    unlock_calls: u32,
    before: Vec<u32>,
    after: Vec<u32>,
    events: Vec<Event>,
}

impl Fixture {
    /// `setup()`: a zeroed encoder with an initialized mutex.
    fn new() -> Self {
        Self {
            initialized: true,
            ..Self::uninitialized()
        }
    }

    /// `struct obs_encoder uninitialized = {0}`.
    fn uninitialized() -> Self {
        Self {
            initialized: false,
            list: Vec::new(),
            capacity: 0,
            reallocations: 0,
            locked: false,
            lock_calls: 0,
            unlock_calls: 0,
            before: Vec::new(),
            after: Vec::new(),
            events: Vec::new(),
        }
    }
}

impl Outputs for Fixture {
    type Output = u32;
    fn lock(&mut self) {
        assert!(self.initialized, "an uninitialized mutex is never locked");
        assert!(!self.locked);
        assert_eq!(self.list, self.before);
        self.locked = true;
        self.lock_calls += 1;
        self.events.push(Event::Lock);
    }
    fn unlock(&mut self) {
        assert!(self.locked);
        assert_eq!(self.list, self.after);
        self.locked = false;
        self.unlock_calls += 1;
        self.events.push(Event::Unlock);
    }
    fn push(&mut self, output: u32) {
        assert!(self.locked, "append under the lock");
        // darray_ensure_capacity(): the first growth to the size, then double.
        let size = self.list.len() + 1;
        if size > self.capacity {
            self.capacity = if self.capacity == 0 {
                size
            } else {
                (self.capacity * 2).max(size)
            };
            self.reallocations += 1;
        }
        self.list.push(output);
        self.events.push(Event::Push(output));
    }
    fn count(&mut self) -> usize {
        assert!(self.locked, "read under the lock");
        self.events.push(Event::Count);
        self.list.len()
    }
    fn at(&mut self, index: usize) -> Option<u32> {
        assert!(self.locked, "read under the lock");
        assert!(index < self.list.len());
        self.events.push(Event::At(index));
        self.list.get(index).copied()
    }
    fn erase(&mut self, index: usize) {
        assert!(self.locked, "erase under the lock");
        self.list.remove(index);
        self.events.push(Event::Erase(index));
    }
}

const A: u32 = 0;
const B: u32 = 1;
const C: u32 = 2;
const D: u32 = 3;

/// `check_list()`.
fn check_list(fixture: &Fixture, expected: &[u32]) {
    assert_eq!(fixture.list.len(), expected.len());
    assert_eq!(fixture.list, expected);
}

/// `change_output()`: the lock must see the list as it was, the unlock the
/// expected list, each once, and the mutex is released afterwards.
fn change_output(fixture: &mut Fixture, add: bool, output: u32, expected: &[u32]) {
    assert!(fixture.list.len() <= 8);
    assert!(expected.len() <= 8);
    fixture.before = fixture.list.clone();
    fixture.after = expected.to_vec();
    fixture.lock_calls = 0;
    fixture.unlock_calls = 0;
    if add {
        add_output(Some(&mut *fixture), Some(output));
    } else {
        remove_output(Some(&mut *fixture), Some(output));
    }
    assert_eq!(fixture.lock_calls, 1);
    assert_eq!(fixture.unlock_calls, 1);
    assert!(!fixture.locked);
    check_list(fixture, expected);
}

/// `teardown()`.
fn teardown(fixture: &Fixture) {
    assert!(!fixture.locked);
}

#[test]
fn characterization_append_and_remove_exact_output() {
    let mut fixture = Fixture::new();
    change_output(&mut fixture, true, A, &[A]);
    change_output(&mut fixture, true, B, &[A, B]);
    change_output(&mut fixture, true, C, &[A, B, C]);
    change_output(&mut fixture, false, B, &[A, C]);
    change_output(&mut fixture, false, A, &[C]);
    change_output(&mut fixture, false, C, &[]);
    teardown(&fixture);
}

#[test]
fn characterization_absent_output_is_unchanged() {
    let mut fixture = Fixture::new();
    change_output(&mut fixture, false, D, &[]);
    change_output(&mut fixture, true, A, &[A]);
    change_output(&mut fixture, true, B, &[A, B]);
    let array = fixture.reallocations;
    let capacity = fixture.capacity;
    change_output(&mut fixture, false, D, &[A, B]);
    assert_eq!(fixture.reallocations, array);
    assert_eq!(fixture.capacity, capacity);
    teardown(&fixture);
}

#[test]
fn characterization_duplicates_remove_first_match_only() {
    // Duplicate entries and first-match erase are characterized, not endorsed.
    let mut fixture = Fixture::new();
    change_output(&mut fixture, true, A, &[A]);
    change_output(&mut fixture, true, B, &[A, B]);
    change_output(&mut fixture, true, A, &[A, B, A]);
    change_output(&mut fixture, false, A, &[B, A]);
    change_output(&mut fixture, false, A, &[B]);
    change_output(&mut fixture, false, A, &[B]);
    teardown(&fixture);
}

#[test]
fn characterization_null_arguments_do_not_lock_or_mutate() {
    let mut fixture = Fixture::new();
    // An uninitialized mutex is safe here: NULL output must return first.
    let mut uninitialized = Fixture::uninitialized();
    add_output::<Fixture>(None, None);
    remove_output::<Fixture>(None, None);
    add_output::<Fixture>(None, Some(A));
    remove_output::<Fixture>(None, Some(A));
    add_output(Some(&mut uninitialized), None);
    remove_output(Some(&mut uninitialized), None);
    assert_eq!(fixture.lock_calls + uninitialized.lock_calls, 0);
    assert_eq!(fixture.unlock_calls + uninitialized.unlock_calls, 0);
    assert_eq!(uninitialized.reallocations, 0);
    assert!(uninitialized.list.is_empty());
    assert_eq!(uninitialized.capacity, 0);
    assert!(uninitialized.events.is_empty());

    change_output(&mut fixture, true, A, &[A]);
    let array = fixture.reallocations;
    let capacity = fixture.capacity;
    fixture.lock_calls = 0;
    fixture.unlock_calls = 0;
    add_output(Some(&mut fixture), None);
    remove_output(Some(&mut fixture), None);
    assert_eq!(fixture.lock_calls, 0);
    assert_eq!(fixture.unlock_calls, 0);
    check_list(&fixture, &[A]);
    assert_eq!(fixture.reallocations, array);
    assert_eq!(fixture.capacity, capacity);
    teardown(&fixture);
}

/// Every event of one call: the lock first, the unlock last, and nothing
/// else outside them.
fn inside_lock_pair(events: &[Event]) -> &[Event] {
    assert_eq!(events.first(), Some(&Event::Lock));
    assert_eq!(events.last(), Some(&Event::Unlock));
    let inner = &events[1..events.len() - 1];
    assert!(!inner.contains(&Event::Lock));
    assert!(!inner.contains(&Event::Unlock));
    inner
}

#[test]
fn edge_append_is_one_push_under_the_lock() {
    let mut fixture = Fixture::new();
    for (output, expected) in [(A, &[A][..]), (A, &[A, A]), (B, &[A, A, B])] {
        fixture.events.clear();
        change_output(&mut fixture, true, output, expected);
        assert_eq!(inside_lock_pair(&fixture.events), [Event::Push(output)]);
    }
}

#[test]
fn edge_removal_erases_the_first_match_and_stops() {
    let mut fixture = Fixture::new();
    for (output, expected) in [(A, &[A][..]), (B, &[A, B]), (A, &[A, B, A])] {
        change_output(&mut fixture, true, output, expected);
    }
    fixture.events.clear();
    change_output(&mut fixture, false, B, &[A, A]);
    let inner = inside_lock_pair(&fixture.events);
    assert_eq!(
        inner
            .iter()
            .filter(|e| matches!(e, Event::Erase(_)))
            .count(),
        1
    );
    assert_eq!(inner.last(), Some(&Event::Erase(1)));
    assert!(
        !inner.contains(&Event::At(2)),
        "the walk stops at the match"
    );

    // Absent: every entry read, nothing erased.
    fixture.events.clear();
    change_output(&mut fixture, false, D, &[A, A]);
    let inner = inside_lock_pair(&fixture.events);
    assert!(inner.contains(&Event::At(0)) && inner.contains(&Event::At(1)));
    assert!(
        !inner
            .iter()
            .any(|e| matches!(e, Event::Erase(_) | Event::Push(_)))
    );

    // Empty: locked and unlocked, nothing else changed.
    let mut empty = Fixture::new();
    change_output(&mut empty, false, A, &[]);
    let inner = inside_lock_pair(&empty.events);
    assert!(
        !inner
            .iter()
            .any(|e| matches!(e, Event::Erase(_) | Event::Push(_) | Event::At(_)))
    );
}

#[derive(Clone, Copy, Debug)]
struct Op {
    remove: bool,
    encoder: bool,
    output: Option<u32>,
}

fn op() -> impl Strategy<Value = Op> {
    (
        any::<bool>(),
        prop::bool::weighted(0.9),
        prop::option::weighted(0.9, 0..4u32),
    )
        .prop_map(|(remove, encoder, output)| Op {
            remove,
            encoder,
            output,
        })
}

proptest! {
    #[test]
    fn edge_any_sequence_matches_the_c_model(ops in prop::collection::vec(op(), 0..24)) {
        let mut fixture = Fixture::new();
        let mut model: Vec<u32> = Vec::new();
        for op in ops {
            let mut expected = model.clone();
            let changes = op.encoder && op.output.is_some();
            if let (true, Some(output)) = (changes, op.output) {
                if op.remove {
                    if let Some(index) = expected.iter().position(|&o| o == output) {
                        expected.remove(index);
                    }
                } else {
                    expected.push(output);
                }
            }
            fixture.before = model.clone();
            fixture.after = expected.clone();
            fixture.lock_calls = 0;
            fixture.unlock_calls = 0;
            let encoder = if op.encoder { Some(&mut fixture) } else { None };
            if op.remove {
                remove_output(encoder, op.output);
            } else {
                add_output(encoder, op.output);
            }
            let calls = u32::from(changes);
            prop_assert_eq!(fixture.lock_calls, calls);
            prop_assert_eq!(fixture.unlock_calls, calls);
            prop_assert!(!fixture.locked);
            prop_assert_eq!(&fixture.list, &expected);
            model = expected;
        }
    }
}
