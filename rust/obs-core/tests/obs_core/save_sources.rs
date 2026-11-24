//! Safe mirrors of test_save_sources.c over a recording host: the core's
//! uuid table in creation order (each source's next link), a sources mutex,
//! each source's filter type and `removed`, `temp_removed` (hidden) and
//! `private` fields, and data calls that hand out numbered arrays and saves
//! and count each save's pushes and releases. Every case of the C test is
//! mirrored, with the sources it creates.
//!
//! Plus what the C test cannot see: the exact order of every read and call
//! (the array made before the lock, the head read once under it, the four
//! checks in the C's order with each one made only when the ones before it
//! passed, then the callback, then the save, push and release, the next link
//! read after the source is handled, the mutex held throughout and released
//! once), a source both removed and private, a callback whose answer for one
//! source changes between calls, and edits made from the callback.
use obs_core::save_sources::{Sources, save_sources, save_sources_filtered};

const FILTER: u8 = 1;
const REMOVED: u8 = 2;
const HIDDEN: u8 = 4;
const PRIVATE: u8 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    Create(usize),
    Lock,
    Unlock,
    First,
    Next(usize),
    IsFilter(usize),
    Removed(usize),
    Hidden(usize),
    Private(usize),
    Save(usize),
    Push {
        array: usize,
        data: usize,
    },
    Release(usize),
    /// A step made without the mutex held (other than the array's creation,
    /// which must be made without it).
    Unlocked,
    /// The array made under the mutex.
    ArrayLocked,
}

#[derive(Debug, Clone)]
struct Source {
    flags: u8,
    next: Option<usize>,
}

/// One save: its source, its pushes (array numbers) and its releases.
#[derive(Debug, Clone, Default)]
struct Save {
    source: usize,
    pushed: Vec<usize>,
    released: usize,
}

#[derive(Debug, Default)]
struct Host {
    sources: Vec<Source>,
    head: Option<usize>,
    depth: usize,
    arrays: usize,
    saves: Vec<Save>,
    events: Vec<Event>,
}

impl Host {
    /// A table of `flags.len()` sources in creation order.
    fn with(flags: &[u8]) -> Self {
        let mut host = Host::default();
        for &f in flags {
            host.add(f);
        }
        host
    }

    /// Creates a source at the table's tail, as obs_source_create does.
    fn add(&mut self, flags: u8) -> usize {
        let id = self.sources.len();
        self.sources.push(Source { flags, next: None });
        match self.tail() {
            Some(tail) => self.sources[tail].next = Some(id),
            None => self.head = Some(id),
        }
        id
    }

    fn tail(&self) -> Option<usize> {
        let mut at = self.head?;
        while let Some(next) = self.sources[at].next {
            at = next;
        }
        Some(at)
    }

    /// Unlinks a source from the table, as its last release does.
    fn unlist(&mut self, id: usize) {
        let next = self.sources[id].next;
        if self.head == Some(id) {
            self.head = next;
        } else if let Some(prev) =
            (0..self.sources.len()).find(|&i| self.sources[i].next == Some(id))
        {
            self.sources[prev].next = next;
        }
        self.sources[id].next = None;
    }

    fn check(&mut self) {
        if self.depth == 0 {
            self.events.push(Event::Unlocked);
        }
    }

    fn record(&mut self, event: Event) {
        self.check();
        self.events.push(event);
    }

    fn flag(&mut self, id: usize, bit: u8, event: Event) -> bool {
        self.record(event);
        self.sources[id].flags & bit != 0
    }

    /// The sources of `array`'s pushes, in push order.
    fn saved_in(&self, array: usize) -> Vec<usize> {
        let mut pushes: Vec<(usize, usize)> = Vec::new();
        for (i, event) in self.events.iter().enumerate() {
            if let Event::Push { array: a, data } = *event
                && a == array
            {
                pushes.push((i, self.saves[data].source));
            }
        }
        pushes.into_iter().map(|(_, source)| source).collect()
    }

    /// Every save pushed once and released once, after its push.
    fn assert_balanced(&self) {
        for save in &self.saves {
            assert_eq!(save.pushed.len(), 1, "{save:?}");
            assert_eq!(save.released, 1, "{save:?}");
        }
        for (i, event) in self.events.iter().enumerate() {
            if let Event::Release(data) = *event {
                let pushed = self.events[..i]
                    .iter()
                    .any(|e| matches!(*e, Event::Push { data: d, .. } if d == data));
                assert!(pushed, "save {data} released before its push");
            }
        }
        assert!(!self.events.contains(&Event::Unlocked), "{:?}", self.events);
        assert!(
            !self.events.contains(&Event::ArrayLocked),
            "{:?}",
            self.events
        );
        assert_eq!(self.depth, 0);
    }
}

impl Sources for Host {
    type Source = usize;
    type Data = usize;
    type Array = usize;
    fn array_create(&mut self) -> usize {
        if self.depth != 0 {
            self.events.push(Event::ArrayLocked);
        }
        let n = self.arrays;
        self.arrays += 1;
        self.events.push(Event::Create(n));
        n
    }
    fn lock(&mut self) {
        self.events.push(Event::Lock);
        self.depth += 1;
    }
    fn unlock(&mut self) {
        self.check();
        self.events.push(Event::Unlock);
        self.depth = self.depth.saturating_sub(1);
    }
    fn first(&mut self) -> Option<usize> {
        self.record(Event::First);
        self.head
    }
    fn next(&mut self, source: usize) -> Option<usize> {
        self.record(Event::Next(source));
        self.sources[source].next
    }
    fn is_filter(&mut self, source: usize) -> bool {
        self.flag(source, FILTER, Event::IsFilter(source))
    }
    fn removed(&mut self, source: usize) -> bool {
        self.flag(source, REMOVED, Event::Removed(source))
    }
    fn temp_removed(&mut self, source: usize) -> bool {
        self.flag(source, HIDDEN, Event::Hidden(source))
    }
    fn private(&mut self, source: usize) -> bool {
        self.flag(source, PRIVATE, Event::Private(source))
    }
    fn save(&mut self, source: usize) -> usize {
        self.record(Event::Save(source));
        self.saves.push(Save {
            source,
            ..Save::default()
        });
        self.saves.len() - 1
    }
    fn push(&mut self, array: usize, data: usize) {
        self.record(Event::Push { array, data });
        self.saves[data].pushed.push(array);
    }
    fn release(&mut self, data: usize) {
        self.record(Event::Release(data));
        self.saves[data].released += 1;
    }
}

/// Runs `obs_save_sources_filtered` with a callback that records each
/// source it is called for and keeps the i-th when bit i of `mask` is set.
fn filtered(host: &mut Host, mask: u32) -> (usize, Vec<usize>) {
    let mut seen = Vec::new();
    let array = save_sources_filtered(host, |source| {
        let index = seen.len();
        seen.push(source);
        index < 32 && mask & (1 << index) != 0
    });
    (array, seen)
}

// The C test's cases.

#[test]
fn empty_core_returns_new_empty_arrays() {
    let mut host = Host::default();
    let first = save_sources(&mut host);
    let (filtered_array, seen) = filtered(&mut host, u32::MAX);
    let second = save_sources(&mut host);
    assert!(seen.is_empty());
    assert_ne!(filtered_array, first);
    assert_ne!(second, first);
    assert_ne!(second, filtered_array);
    for array in [first, filtered_array, second] {
        assert!(host.saved_in(array).is_empty());
    }
    assert!(host.saves.is_empty());
    host.assert_balanced();
}

#[test]
fn saves_public_inputs_and_scenes_in_creation_order() {
    // input a, scene a, input b, scene b
    let mut host = Host::with(&[0, 0, 0, 0]);
    let array = save_sources(&mut host);
    assert_eq!(host.saved_in(array), [0, 1, 2, 3]);
    host.assert_balanced();
    // All released: nothing listed, nothing saved.
    for id in 0..4 {
        host.unlist(id);
    }
    let array = save_sources(&mut host);
    assert!(host.saved_in(array).is_empty());
}

#[test]
fn leaves_out_filters_private_hidden_removed_and_released() {
    let mut host = Host::default();
    let first = host.add(0);
    // obs_source_create always makes a filter private.
    let _attached_filter = host.add(FILTER | PRIVATE);
    let _loose_filter = host.add(FILTER | PRIVATE);
    let _private_input = host.add(PRIVATE);
    let _private_scene = host.add(PRIVATE);
    let hidden = host.add(HIDDEN);
    let _removed = host.add(REMOVED);
    let released = host.add(0);
    let middle = host.add(0);
    let last = host.add(0);
    host.unlist(released);

    let array = save_sources(&mut host);
    assert_eq!(host.saved_in(array), [first, middle, last]);

    // A hidden source that is shown again is saved again, in its place.
    host.sources[hidden].flags = 0;
    let array = save_sources(&mut host);
    assert_eq!(host.saved_in(array), [first, hidden, middle, last]);
    host.assert_balanced();
}

#[test]
fn filtered_calls_back_per_candidate_and_keeps_true() {
    let mut host = Host::default();
    let a = host.add(0);
    host.add(PRIVATE);
    let b = host.add(0);
    host.add(FILTER | PRIVATE);
    host.add(REMOVED);
    host.add(HIDDEN);
    let c = host.add(0);
    let d = host.add(0);
    let candidates = [a, b, c, d];

    for mask in [0x5_u32, 0x0, 0xf, 0xa] {
        let (array, seen) = filtered(&mut host, mask);
        assert_eq!(seen, candidates);
        let kept: Vec<usize> = (0..4)
            .filter(|i| mask & (1 << i) != 0)
            .map(|i| candidates[i])
            .collect();
        assert_eq!(host.saved_in(array), kept, "mask {mask:#x}");
    }
    host.assert_balanced();
}

#[test]
fn save_keeps_refcount() {
    // The safe core has no reference calls at all: every save it makes is
    // pushed and released exactly once, and nothing else touches a source.
    let mut host = Host::with(&[0, 0]);
    for _ in 0..3 {
        let array = save_sources(&mut host);
        assert_eq!(host.saved_in(array).len(), 2);
        let (array, _) = filtered(&mut host, 0x3);
        assert_eq!(host.saved_in(array).len(), 2);
    }
    assert_eq!(host.saves.len(), 12);
    host.assert_balanced();
}

// What the C test cannot see.

#[test]
fn reads_and_calls_in_the_c_order() {
    use Event::*;
    // a kept, b private, c refused by the callback, d removed and private, e
    // a filter, f hidden.
    let mut host = Host::with(&[0, PRIVATE, 0, REMOVED | PRIVATE, FILTER, HIDDEN]);
    let (array, seen) = filtered(&mut host, 0b01);
    assert_eq!(array, 0);
    assert_eq!(seen, [0, 2]);
    assert_eq!(
        host.events,
        [
            Create(0),
            Lock,
            First,
            IsFilter(0),
            Removed(0),
            Hidden(0),
            Private(0),
            Save(0),
            Push { array: 0, data: 0 },
            Release(0),
            Next(0),
            IsFilter(1),
            Removed(1),
            Hidden(1),
            Private(1),
            Next(1),
            IsFilter(2),
            Removed(2),
            Hidden(2),
            Private(2),
            Next(2),
            IsFilter(3),
            Removed(3),
            Next(3),
            IsFilter(4),
            Next(4),
            IsFilter(5),
            Removed(5),
            Hidden(5),
            Next(5),
            Unlock,
        ]
    );
    host.assert_balanced();
}

#[test]
fn an_empty_table_makes_the_array_and_takes_the_lock() {
    use Event::*;
    let mut host = Host::default();
    assert_eq!(save_sources(&mut host), 0);
    assert_eq!(host.events, [Create(0), Lock, First, Unlock]);
}

#[test]
fn a_callback_that_changes_its_answer() {
    let mut host = Host::with(&[0, 0, 0]);
    let mut calls = 0_u32;
    let mut run = |host: &mut Host| {
        save_sources_filtered(host, |_| {
            calls += 1;
            // keep on odd calls: a, c, then b
            calls % 2 == 1
        })
    };
    let first = run(&mut host);
    let second = run(&mut host);
    assert_eq!(host.saved_in(first), [0, 2]);
    assert_eq!(host.saved_in(second), [1]);
    host.assert_balanced();
}

/// A walk that edits the table while it saves: the edit is seen where the C
/// would see it.
#[test]
fn edits_from_the_callback_are_seen_by_later_reads() {
    // The callback cannot borrow the host the walk holds, so the edit goes
    // through the save of a: b marked removed, or a relinked past b.
    let mut host = Host::with(&[0, 0, 0]);
    struct Editing<'a>(&'a mut Host, bool);
    impl Sources for Editing<'_> {
        type Source = usize;
        type Data = usize;
        type Array = usize;
        fn array_create(&mut self) -> usize {
            self.0.array_create()
        }
        fn lock(&mut self) {
            self.0.lock();
        }
        fn unlock(&mut self) {
            self.0.unlock();
        }
        fn first(&mut self) -> Option<usize> {
            self.0.first()
        }
        fn next(&mut self, s: usize) -> Option<usize> {
            self.0.next(s)
        }
        fn is_filter(&mut self, s: usize) -> bool {
            self.0.is_filter(s)
        }
        fn removed(&mut self, s: usize) -> bool {
            self.0.removed(s)
        }
        fn temp_removed(&mut self, s: usize) -> bool {
            self.0.temp_removed(s)
        }
        fn private(&mut self, s: usize) -> bool {
            self.0.private(s)
        }
        fn save(&mut self, s: usize) -> usize {
            let data = self.0.save(s);
            if s == 0 {
                if self.1 {
                    // relink a past b
                    self.0.sources[0].next = Some(2);
                } else {
                    self.0.sources[1].flags = REMOVED;
                }
            }
            data
        }
        fn push(&mut self, a: usize, d: usize) {
            self.0.push(a, d);
        }
        fn release(&mut self, d: usize) {
            self.0.release(d);
        }
    }
    let array = save_sources(&mut Editing(&mut host, false));
    assert_eq!(host.saved_in(array), [0, 2]);

    let mut host = Host::with(&[0, 0, 0]);
    let array = save_sources(&mut Editing(&mut host, true));
    assert_eq!(host.saved_in(array), [0, 2]);
    assert!(!host.events.contains(&Event::IsFilter(1)));
    host.assert_balanced();
}
