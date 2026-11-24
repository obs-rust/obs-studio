//! Safe mirrors of test_load_sources.c over a recording host: a data array
//! of entries, each loading a numbered source (or NULL), each source an
//! input or a transition, and data and source references counted per item
//! and per source. Every case of the C test is mirrored, with the entries it
//! loads (a missing-plugin entry is a source like any other here: its
//! missing load is inside `obs_source_load2`, which stays C).
//!
//! Plus what the C test cannot see: the exact order of every call (the count
//! read once; pass 1 taking, loading and releasing each item in turn; pass 2
//! taking each item again, reading the type, the transition load, the
//! second-pass load and the callback, then the release; pass 3 releasing
//! every slot), NULL slots (their item still taken and released, their NULL
//! released in pass 3), a NULL callback, an empty and a NULL array, and a
//! type changed by an earlier source's second-pass load.
use std::cell::RefCell;
use std::rc::Rc;

use obs_core::load_sources::{Loader, load_sources};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    Count,
    Item(usize),
    ReleaseData(Option<usize>),
    Load(Option<usize>),
    IsTransition(usize),
    TransitionLoad { source: usize, data: Option<usize> },
    Load2(usize),
    Callback(usize),
    ReleaseSource(Option<usize>),
}

use Event::*;

/// The array under load: `None` for a NULL array.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Array(Option<usize>);

#[derive(Debug, Default)]
struct Host {
    /// Per entry: the source its load makes, or NULL.
    entries: Vec<Option<usize>>,
    /// Per source: a transition.
    transition: Vec<bool>,
    data_refs: Vec<i32>,
    source_refs: Vec<i32>,
    /// On second-pass load call `.0`, flip source `.1`'s type.
    flip: Option<(usize, usize)>,
    load2_calls: usize,
    events: Vec<Event>,
}

impl Host {
    /// `entries` loading the sources given (`None`: NULL), `transitions` the
    /// sources that are transitions.
    fn new(entries: &[Option<usize>], transitions: &[usize]) -> Self {
        let sources = entries.iter().flatten().map(|&s| s + 1).max().unwrap_or(0);
        let mut transition = vec![false; sources];
        for &t in transitions {
            transition[t] = true;
        }
        Host {
            entries: entries.to_vec(),
            transition,
            data_refs: vec![0; entries.len()],
            source_refs: vec![0; sources],
            ..Host::default()
        }
    }

    fn balanced(&self) -> bool {
        self.data_refs.iter().all(|&r| r == 0) && self.source_refs.iter().all(|&r| r == 0)
    }

    fn position(&self, event: Event) -> usize {
        self.events.iter().position(|&e| e == event).unwrap()
    }
}

impl Loader for Host {
    type Array = Array;
    type Data = Option<usize>;
    type Source = usize;
    fn count(&mut self, array: Array) -> usize {
        self.events.push(Count);
        array.0.map_or(0, |_| self.entries.len())
    }
    fn item(&mut self, array: Array, index: usize) -> Option<usize> {
        self.events.push(Item(index));
        array.0?;
        if index >= self.entries.len() {
            return None;
        }
        self.data_refs[index] += 1;
        Some(index)
    }
    fn release_data(&mut self, data: Option<usize>) {
        self.events.push(ReleaseData(data));
        if let Some(i) = data {
            assert!(self.data_refs[i] > 0, "item {i} released twice");
            self.data_refs[i] -= 1;
        }
    }
    fn load(&mut self, data: Option<usize>) -> Option<usize> {
        self.events.push(Load(data));
        let i = data?;
        assert!(self.data_refs[i] > 0, "item {i} loaded without a reference");
        let source = self.entries[i]?;
        self.source_refs[source] += 1;
        Some(source)
    }
    fn is_transition(&mut self, source: usize) -> bool {
        self.events.push(IsTransition(source));
        self.transition[source]
    }
    fn transition_load(&mut self, source: usize, data: Option<usize>) {
        self.events.push(TransitionLoad { source, data });
        assert!(self.source_refs[source] > 0);
    }
    fn load2(&mut self, source: usize) {
        self.events.push(Load2(source));
        assert!(self.source_refs[source] > 0);
        if let Some((call, target)) = self.flip
            && call == self.load2_calls
        {
            self.transition[target] = !self.transition[target];
        }
        self.load2_calls += 1;
    }
    fn release_source(&mut self, source: Option<usize>) {
        self.events.push(ReleaseSource(source));
        if let Some(s) = source {
            assert!(self.source_refs[s] > 0, "source {s} released twice");
            self.source_refs[s] -= 1;
        }
    }
}

const ARRAY: Array = Array(Some(0));

/// The host shared with the callback, which records itself in the host's
/// trace and checks its source is still held.
struct Shared(Rc<RefCell<Host>>);

impl Loader for Shared {
    type Array = Array;
    type Data = Option<usize>;
    type Source = usize;
    fn count(&mut self, array: Array) -> usize {
        self.0.borrow_mut().count(array)
    }
    fn item(&mut self, array: Array, index: usize) -> Option<usize> {
        self.0.borrow_mut().item(array, index)
    }
    fn release_data(&mut self, data: Option<usize>) {
        self.0.borrow_mut().release_data(data);
    }
    fn load(&mut self, data: Option<usize>) -> Option<usize> {
        self.0.borrow_mut().load(data)
    }
    fn is_transition(&mut self, source: usize) -> bool {
        self.0.borrow_mut().is_transition(source)
    }
    fn transition_load(&mut self, source: usize, data: Option<usize>) {
        self.0.borrow_mut().transition_load(source, data);
    }
    fn load2(&mut self, source: usize) {
        self.0.borrow_mut().load2(source);
    }
    fn release_source(&mut self, source: Option<usize>) {
        self.0.borrow_mut().release_source(source);
    }
}

/// Runs the load with the recording callback; returns the host.
fn run(host: Host, array: Array) -> Host {
    let host = Rc::new(RefCell::new(host));
    let seen = Rc::clone(&host);
    load_sources(
        &mut Shared(Rc::clone(&host)),
        array,
        Some(move |source: usize| {
            let mut h = seen.borrow_mut();
            assert!(h.source_refs[source] > 0, "callback for a source not held");
            h.events.push(Callback(source));
        }),
    );
    Rc::try_unwrap(host).ok().unwrap().into_inner()
}

/// The callback's sources, in order.
fn callbacks(host: &Host) -> Vec<usize> {
    host.events
        .iter()
        .filter_map(|e| match e {
            Callback(s) => Some(*s),
            _ => None,
        })
        .collect()
}

#[test]
fn empty_array_creates_nothing_and_never_calls_back() {
    let host = Host::new(&[], &[]);
    let host = run(host, ARRAY);
    let calls = callbacks(&host);
    assert!(calls.is_empty());
    assert_eq!(host.events, [Count]);
    assert!(host.balanced());
}

#[test]
fn null_array_reads_the_count_only() {
    let host = Host::new(&[Some(0)], &[]);
    let host = run(host, Array(None));
    let calls = callbacks(&host);
    assert!(calls.is_empty());
    assert_eq!(host.events, [Count]);
}

#[test]
fn creates_all_then_loads_and_calls_back_in_order() {
    // The C test's array: three inputs, a missing-plugin entry, the
    // transition's source A, then the transition (whose source A comes
    // earlier, found because every entry exists before any load).
    let entries: Vec<Option<usize>> = (0..6).map(Some).collect();
    let host = Host::new(&entries, &[5]);
    let host = run(host, ARRAY);
    let calls = callbacks(&host);
    assert_eq!(calls, [0, 1, 2, 3, 4, 5]);
    // Every load before the first second-pass load.
    let last_load = host.position(Load(Some(5)));
    assert!(last_load < host.position(Load2(0)));
    // Each source's load2 then its callback, in array order.
    for s in 0..6 {
        assert!(host.position(Load2(s)) < host.position(Callback(s)));
    }
    // The transition loads its transition data before its own load.
    let t = host.position(TransitionLoad {
        source: 5,
        data: Some(5),
    });
    assert!(t < host.position(Load2(5)));
    assert_eq!(
        host.events
            .iter()
            .filter(|e| matches!(e, TransitionLoad { .. }))
            .count(),
        1
    );
    assert!(host.balanced());
}

#[test]
fn callback_follows_each_load2() {
    let host = run(Host::new(&[Some(0), Some(1), Some(2)], &[]), ARRAY);
    for s in 0..3 {
        let at = host.position(Callback(s));
        assert_eq!(host.events[at - 1], Load2(s));
        assert_eq!(host.events[at + 1], ReleaseData(Some(s)));
    }
}

#[test]
fn null_cb_still_loads() {
    let mut host = Host::new(&[Some(0), Some(1)], &[]);
    load_sources(&mut host, ARRAY, None::<fn(usize)>);
    assert_eq!(
        host.events,
        [
            Count,
            Item(0),
            Load(Some(0)),
            ReleaseData(Some(0)),
            Item(1),
            Load(Some(1)),
            ReleaseData(Some(1)),
            Item(0),
            IsTransition(0),
            Load2(0),
            ReleaseData(Some(0)),
            Item(1),
            IsTransition(1),
            Load2(1),
            ReleaseData(Some(1)),
            ReleaseSource(Some(0)),
            ReleaseSource(Some(1)),
        ]
    );
    assert!(host.balanced());
}

#[test]
fn call_leaves_no_reference_behind() {
    let host = Host::new(&[Some(0), Some(1), Some(2)], &[]);
    let host = run(host, ARRAY);
    let calls = callbacks(&host);
    assert_eq!(calls, [0, 1, 2]);
    assert!(host.balanced());
    // Every source released once, after every callback, in array order.
    let releases: Vec<Event> = host
        .events
        .iter()
        .copied()
        .filter(|e| matches!(e, ReleaseSource(_)))
        .collect();
    assert_eq!(
        releases,
        [
            ReleaseSource(Some(0)),
            ReleaseSource(Some(1)),
            ReleaseSource(Some(2))
        ]
    );
    assert!(host.position(ReleaseSource(Some(0))) > host.position(Load2(2)));
}

#[test]
fn exact_call_order_with_a_transition_and_a_null_slot() {
    let host = Host::new(&[Some(0), None, Some(1)], &[1]);
    let host = run(host, ARRAY);
    let calls = callbacks(&host);
    assert_eq!(calls, [0, 1]);
    assert_eq!(
        host.events,
        [
            Count,
            Item(0),
            Load(Some(0)),
            ReleaseData(Some(0)),
            Item(1),
            Load(Some(1)),
            ReleaseData(Some(1)),
            Item(2),
            Load(Some(2)),
            ReleaseData(Some(2)),
            Item(0),
            IsTransition(0),
            Load2(0),
            Callback(0),
            ReleaseData(Some(0)),
            // The NULL slot: its item taken and released, nothing else.
            Item(1),
            ReleaseData(Some(1)),
            Item(2),
            IsTransition(1),
            TransitionLoad {
                source: 1,
                data: Some(2)
            },
            Load2(1),
            Callback(1),
            ReleaseData(Some(2)),
            ReleaseSource(Some(0)),
            // The NULL slot released too, as obs_source_release(NULL).
            ReleaseSource(None),
            ReleaseSource(Some(1)),
        ]
    );
    assert!(host.balanced());
}

#[test]
fn every_slot_null() {
    let host = Host::new(&[None, None], &[]);
    let host = run(host, ARRAY);
    let calls = callbacks(&host);
    assert!(calls.is_empty());
    assert_eq!(
        host.events,
        [
            Count,
            Item(0),
            Load(Some(0)),
            ReleaseData(Some(0)),
            Item(1),
            Load(Some(1)),
            ReleaseData(Some(1)),
            Item(0),
            ReleaseData(Some(0)),
            Item(1),
            ReleaseData(Some(1)),
            ReleaseSource(None),
            ReleaseSource(None),
        ]
    );
}

#[test]
fn the_type_is_read_after_earlier_second_pass_loads() {
    // Source 0's load2 makes source 1 a transition: the C reads 1's type in
    // pass 2, after that load2, so 1 gets a transition load.
    let mut host = Host::new(&[Some(0), Some(1)], &[]);
    host.flip = Some((0, 1));
    let host = run(host, ARRAY);
    assert!(host.events.contains(&TransitionLoad {
        source: 1,
        data: Some(1)
    }));
    // And the reverse: a transition made an input is not transition-loaded.
    let mut host = Host::new(&[Some(0), Some(1)], &[1]);
    host.flip = Some((0, 1));
    let host = run(host, ARRAY);
    assert!(
        !host
            .events
            .iter()
            .any(|e| matches!(e, TransitionLoad { .. }))
    );
}

#[test]
fn the_same_source_in_two_slots_is_handled_per_slot() {
    let host = Host::new(&[Some(0), Some(0)], &[0]);
    let host = run(host, ARRAY);
    let calls = callbacks(&host);
    assert_eq!(calls, [0, 0]);
    assert_eq!(
        host.events
            .iter()
            .filter(|e| matches!(e, TransitionLoad { source: 0, .. }))
            .count(),
        2
    );
    assert!(host.balanced());
}
