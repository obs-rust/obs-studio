//! Tier 1: `obs_source::output_cea708` over a recording model of the
//! source, mirroring every `test_source_output_cea708` case plus the order
//! of every read and the lock's scope.
use obs_source::output_cea708::{Locked, Source, output};
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    Destroying,
    Lock,
    Count,
    Call { index: usize, id: u32, locked: bool },
    Unlock,
}

#[derive(Default)]
struct State {
    destroying: bool,
    locked: bool,
    /// Callback ids, in registration order.
    list: Vec<u32>,
    /// Ids a call of this id appends to the list.
    grow: Option<(u32, Vec<u32>)>,
    events: Vec<Event>,
}

struct Model(RefCell<State>);

struct Guard<'a>(&'a Model);

impl Locked for Guard<'_> {
    fn count(&self) -> usize {
        let mut state = self.0.0.borrow_mut();
        state.events.push(Event::Count);
        state.list.len()
    }
    fn call(&mut self, index: usize) {
        let mut state = self.0.0.borrow_mut();
        let id = state.list[index];
        let locked = state.locked;
        state.events.push(Event::Call { index, id, locked });
        if let Some((grower, added)) = state.grow.clone()
            && grower == id
        {
            state.list.extend(added);
        }
    }
}

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        let mut state = self.0.0.borrow_mut();
        assert!(state.locked);
        state.locked = false;
        state.events.push(Event::Unlock);
    }
}

impl Source for Model {
    type Locked<'a> = Guard<'a>;
    fn destroying(&self) -> bool {
        let mut state = self.0.borrow_mut();
        state.events.push(Event::Destroying);
        state.destroying
    }
    fn lock(&self) -> Guard<'_> {
        let mut state = self.0.borrow_mut();
        assert!(!state.locked, "the dispatch locks once");
        state.locked = true;
        state.events.push(Event::Lock);
        Guard(self)
    }
}

fn model(list: &[u32]) -> Model {
    Model(RefCell::new(State {
        list: list.to_vec(),
        ..State::default()
    }))
}

fn call(index: usize, id: u32) -> Event {
    Event::Call {
        index,
        id,
        locked: true,
    }
}

#[test]
fn destroying_returns_first() {
    let source = model(&[1]);
    source.0.borrow_mut().destroying = true;
    output(&source, true);
    output(&source, false);
    let state = source.0.borrow();
    assert_eq!(state.events, [Event::Destroying, Event::Destroying]);
    assert!(!state.locked);
}

#[test]
fn null_captions_returns_unlocked() {
    let source = model(&[1, 2]);
    output(&source, false);
    let state = source.0.borrow();
    assert_eq!(state.events, [Event::Destroying]);
    assert!(!state.locked);
}

#[test]
fn empty_list() {
    let source = model(&[]);
    output(&source, true);
    assert_eq!(
        source.0.borrow().events,
        [Event::Destroying, Event::Lock, Event::Count, Event::Unlock]
    );
}

#[test]
fn newest_first_under_mutex() {
    // a/first, b/first, a/second, a/first, b/NULL as ids 1..=5.
    let source = model(&[1, 2, 3, 1, 4]);
    output(&source, true);
    assert_eq!(
        source.0.borrow().events,
        [
            Event::Destroying,
            Event::Lock,
            Event::Count,
            call(4, 4),
            call(3, 1),
            call(2, 3),
            call(1, 2),
            call(0, 1),
            Event::Unlock,
        ]
    );
    assert_eq!(source.0.borrow().list, [1, 2, 3, 1, 4]);
    source.0.borrow_mut().events.clear();
    output(&source, true);
    assert_eq!(source.0.borrow().events.len(), 9);
}

#[test]
fn growth_during_dispatch() {
    let source = model(&[1, 2, 3]);
    source.0.borrow_mut().grow = Some((3, vec![9; 64]));
    output(&source, true);
    let state = source.0.borrow();
    assert_eq!(
        state.events,
        [
            Event::Destroying,
            Event::Lock,
            Event::Count,
            call(2, 3),
            call(1, 2),
            call(0, 1),
            Event::Unlock,
        ]
    );
    assert_eq!(state.list.len(), 3 + 64);
}

#[test]
fn every_length() {
    for len in 0..40usize {
        let ids: Vec<u32> = (0..len).map(|i| u32::try_from(i).unwrap()).collect();
        let source = model(&ids);
        output(&source, true);
        let state = source.0.borrow();
        let calls: Vec<usize> = state
            .events
            .iter()
            .filter_map(|event| match event {
                Event::Call { index, locked, .. } => {
                    assert!(locked);
                    Some(*index)
                }
                _ => None,
            })
            .collect();
        assert_eq!(calls, (0..len).rev().collect::<Vec<_>>());
        assert_eq!(state.events.first(), Some(&Event::Destroying));
        assert_eq!(state.events.last(), Some(&Event::Unlock));
        assert_eq!(
            state.events.iter().filter(|e| **e == Event::Count).count(),
            1
        );
    }
}
