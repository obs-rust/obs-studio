//! Safe mirrors of test_enum_sources.c over a recording host: one hash table
//! of entries (a source's type, whether its id is the group's, its private
//! flag and its reference count, `-1` for none left), linked head first, with
//! a re-entrant mutex, and a caller callback that records each visit with
//! the entry's count at the call, can stop the walk and can unlink the
//! current entry. Every case of the C test is mirrored on the walk it uses:
//! `obs_enum_sources()` ([`enum_sources`]), `obs_enum_all_sources()`
//! ([`enum_uuid`]), `obs_enum_canvases()` ([`enum_canvases`]) and
//! `obs_canvas_enum_scenes()` ([`canvas_enum_scenes`]).
//!
//! Plus what the C test cannot see: the exact order of every read and call
//! (the head and each next read under the lock, except the canvases' head,
//! which the original reads before locking; a reference released before the
//! next link is read; the private flag, input type and group id asked of a
//! referenced source in that order), uthash's next read before each
//! callback of the unreferenced walks, an input whose id is the group's
//! called back twice, and the scene type asked of every referenced entry of
//! a canvas.
use obs_core::enum_sources::{
    Referenced, Scenes, Sources, Table, canvas_enum_scenes, enum_canvases, enum_sources, enum_uuid,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Input,
    Scene,
    Filter,
    Transition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    Lock,
    Unlock,
    /// The head read, with the mutex's hold count at the read.
    First(u32),
    Next(usize),
    GetRef(usize, bool),
    Release(usize),
    Private(usize),
    Input(usize),
    Group(usize),
    Scene(usize),
    Visit(usize),
}

#[derive(Debug, Clone, Copy)]
struct Entry {
    kind: Kind,
    group: bool,
    private: bool,
    /// OBS's count: 0 is one reference, -1 none left.
    refs: i64,
}

fn entry(kind: Kind) -> Entry {
    Entry {
        kind,
        group: false,
        private: false,
        refs: 0,
    }
}

fn private(kind: Kind) -> Entry {
    Entry {
        private: true,
        ..entry(kind)
    }
}

fn group() -> Entry {
    Entry {
        group: true,
        ..entry(Kind::Scene)
    }
}

/// One table, head first, and the callback's behaviour.
struct Host {
    entries: Vec<Entry>,
    head: Option<usize>,
    next: Vec<Option<usize>>,
    depth: u32,
    events: Vec<Event>,
    /// Each visit: the entry and its count at the call.
    seen: Vec<(usize, i64)>,
    /// Answer false at this many visits, as the C test's `stop_after`.
    stop_after: Option<usize>,
    /// Unlink the current entry in its callback at this zero-based visit.
    unlink_at: Option<usize>,
}

impl Host {
    fn new(entries: &[Entry]) -> Self {
        let count = entries.len();
        Host {
            entries: entries.to_vec(),
            head: (count > 0).then_some(0),
            next: (0..count)
                .map(|i| (i + 1 < count).then_some(i + 1))
                .collect(),
            depth: 0,
            events: Vec::new(),
            seen: Vec::new(),
            stop_after: None,
            unlink_at: None,
        }
    }

    fn stopping(entries: &[Entry], stop_after: usize) -> Self {
        Host {
            stop_after: Some(stop_after),
            ..Host::new(entries)
        }
    }

    fn unlink(&mut self, id: usize) {
        let next = self.next[id];
        if self.head == Some(id) {
            self.head = next;
            return;
        }
        let mut at = self.head;
        while let Some(e) = at {
            if self.next[e] == Some(id) {
                self.next[e] = next;
                return;
            }
            at = self.next[e];
        }
    }

    fn visited(&self) -> Vec<usize> {
        self.seen.iter().map(|&(id, _)| id).collect()
    }

    fn refs(&self) -> Vec<i64> {
        self.entries.iter().map(|e| e.refs).collect()
    }

    fn sources(&mut self) {
        enum_sources(self);
        assert_eq!(self.depth, 0, "mutex left held");
    }

    fn all(&mut self) {
        enum_uuid(self);
        assert_eq!(self.depth, 0, "mutex left held");
    }

    fn canvases(&mut self) {
        enum_canvases(self);
        assert_eq!(self.depth, 0, "mutex left held");
    }

    fn scenes(&mut self) {
        canvas_enum_scenes(self);
        assert_eq!(self.depth, 0, "mutex left held");
    }

    fn assert_locked(&self, what: &str) {
        assert!(self.depth > 0, "{what} outside the lock");
    }
}

impl Table for Host {
    type Entry = usize;
    fn lock(&mut self) {
        self.depth += 1;
        self.events.push(Event::Lock);
    }
    fn unlock(&mut self) {
        assert!(self.depth > 0, "unlock without lock");
        self.depth -= 1;
        self.events.push(Event::Unlock);
    }
    fn first(&mut self) -> Option<usize> {
        self.events.push(Event::First(self.depth));
        self.head
    }
    fn next(&mut self, entry: usize) -> Option<usize> {
        self.assert_locked("next read");
        self.events.push(Event::Next(entry));
        self.next[entry]
    }
    fn visit(&mut self, entry: usize) -> bool {
        self.assert_locked("callback");
        self.events.push(Event::Visit(entry));
        let visit = self.seen.len();
        self.seen.push((entry, self.entries[entry].refs));
        if self.unlink_at == Some(visit) {
            self.unlink(entry);
        }
        self.stop_after.is_none_or(|stop| self.seen.len() < stop)
    }
}

impl Referenced for Host {
    fn get_ref(&mut self, entry: usize) -> Option<usize> {
        self.assert_locked("get_ref");
        let e = &mut self.entries[entry];
        let got = e.refs > -1;
        if got {
            e.refs += 1;
        }
        self.events.push(Event::GetRef(entry, got));
        got.then_some(entry)
    }
    fn release(&mut self, entry: usize) {
        self.assert_locked("release");
        let e = &mut self.entries[entry];
        assert!(e.refs > -1, "release of a source with no reference");
        e.refs -= 1;
        self.events.push(Event::Release(entry));
    }
}

impl Sources for Host {
    fn private(&mut self, source: usize) -> bool {
        self.events.push(Event::Private(source));
        self.entries[source].private
    }
    fn input(&mut self, source: usize) -> bool {
        self.events.push(Event::Input(source));
        self.entries[source].kind == Kind::Input
    }
    fn group(&mut self, source: usize) -> bool {
        self.events.push(Event::Group(source));
        self.entries[source].group
    }
}

impl Scenes for Host {
    fn scene(&mut self, source: usize) -> bool {
        self.events.push(Event::Scene(source));
        self.entries[source].kind == Kind::Scene
    }
}

// The C test's populate(), in creation (uuid table) order.
const IN_A: usize = 0;
const GROUP: usize = 4;
const IN_B: usize = 5;
const N_SOURCES: usize = 8;

fn populated() -> [Entry; N_SOURCES] {
    [
        entry(Kind::Input),   // IN_A
        private(Kind::Input), // PRIV_IN
        entry(Kind::Scene),   // SCENE
        entry(Kind::Filter),  // FILTER
        group(),              // GROUP: a scene-type source with the group id
        entry(Kind::Input),   // IN_B
        Entry {
            private: true,
            ..group()
        }, // PRIV_GROUP
        private(Kind::Scene), // PRIV_SCENE
    ]
}

/// The named canvases: main, "x", "y" (the private one is never named).
fn canvases() -> [Entry; 3] {
    [entry(Kind::Scene); 3]
}

#[test]
fn empty_core() {
    let mut sources = Host::new(&[]);
    sources.sources();
    assert!(sources.seen.is_empty());
    assert_eq!(
        sources.events,
        [Event::Lock, Event::First(1), Event::Unlock]
    );

    let mut all = Host::new(&[]);
    all.all();
    assert!(all.seen.is_empty());
    assert_eq!(all.events, [Event::Lock, Event::First(1), Event::Unlock]);

    // The main canvas is always named.
    let mut canvases = Host::new(&[entry(Kind::Scene)]);
    canvases.canvases();
    assert_eq!(canvases.visited(), [0]);
}

#[test]
fn enum_sources_visits_public_inputs_and_groups() {
    let mut host = Host::new(&populated());
    let before = host.refs();
    host.sources();
    assert_eq!(host.visited(), [IN_A, GROUP, IN_B]);
    // One reference held for each call.
    for &(id, refs) in &host.seen {
        assert_eq!(refs, before[id] + 1);
    }
    assert_eq!(host.refs(), before);
}

#[test]
fn enum_sources_stops_on_false() {
    let expected = [IN_A, GROUP, IN_B];
    // Stopping on the input, on the group and on the last one.
    for stop in 1..=3 {
        let mut host = Host::stopping(&populated(), stop);
        let before = host.refs();
        host.sources();
        assert_eq!(host.visited(), expected[..stop]);
        for &(id, refs) in &host.seen {
            assert_eq!(refs, before[id] + 1);
        }
        assert_eq!(host.refs(), before);
        // A stop releases, then unlocks without reading on.
        assert_eq!(
            host.events[host.events.len() - 2..],
            [Event::Release(expected[stop - 1]), Event::Unlock]
        );
    }
}

#[test]
fn enum_sources_skips_source_without_references() {
    let mut sources = populated();
    sources[IN_A].refs = -1;

    let mut host = Host::new(&sources);
    host.sources();
    assert_eq!(host.visited(), [GROUP, IN_B]);
    assert!(host.events.contains(&Event::GetRef(IN_A, false)));
    assert!(!host.events.contains(&Event::Release(IN_A)));

    let mut all = Host::new(&sources);
    all.all();
    assert_eq!(all.visited(), (0..N_SOURCES).collect::<Vec<_>>());
    assert_eq!(all.seen[0], (IN_A, -1));
}

#[test]
fn enum_all_sources_visits_every_source_without_refs() {
    let mut host = Host::new(&populated());
    let before = host.refs();
    host.all();
    assert_eq!(host.visited(), (0..N_SOURCES).collect::<Vec<_>>());
    for &(id, refs) in &host.seen {
        assert_eq!(refs, before[id]);
    }

    for stop in 1..=N_SOURCES {
        let mut s = Host::stopping(&populated(), stop);
        s.all();
        assert_eq!(s.visited(), (0..stop).collect::<Vec<_>>());
        assert_eq!(s.refs(), before);
        assert_eq!(s.events.last(), Some(&Event::Unlock));
    }
}

#[test]
fn enum_sources_skips_transitions() {
    let table = [
        entry(Kind::Input),
        entry(Kind::Transition),
        entry(Kind::Input),
    ];
    let mut host = Host::new(&table);
    host.sources();
    assert_eq!(host.visited(), [0, 2]);

    let mut all = Host::new(&table);
    all.all();
    assert_eq!(all.visited(), [0, 1, 2]);
}

#[test]
fn enum_canvases_visits_named_canvases() {
    let mut host = Host::new(&canvases());
    let before = host.refs();
    host.canvases();
    assert_eq!(host.visited(), [0, 1, 2]);
    // No reference taken for the call.
    for &(id, refs) in &host.seen {
        assert_eq!(refs, before[id]);
    }

    for stop in 1..=3 {
        let mut s = Host::stopping(&canvases(), stop);
        s.canvases();
        assert_eq!(s.visited(), (0..stop).collect::<Vec<_>>());
        assert_eq!(s.refs(), before);
    }

    // A removed canvas stays while it is still named, and leaves the walk
    // once destroyed (unlinked).
    let mut removed = Host::new(&canvases());
    removed.canvases();
    assert_eq!(removed.visited(), [0, 1, 2]);
    let mut destroyed = Host::new(&canvases());
    destroyed.unlink(1);
    destroyed.canvases();
    assert_eq!(destroyed.visited(), [0, 2]);
}

#[test]
fn canvas_enum_scenes_on_other_canvas() {
    let scenes = [entry(Kind::Scene); 3];
    let mut host = Host::new(&scenes);
    let before = host.refs();
    host.scenes();
    assert_eq!(host.visited(), [0, 1, 2]);
    for &(id, refs) in &host.seen {
        assert_eq!(refs, before[id] + 1);
    }

    for stop in 1..=3 {
        let mut s = Host::stopping(&scenes, stop);
        s.scenes();
        assert_eq!(s.visited(), (0..stop).collect::<Vec<_>>());
        for &(id, refs) in &s.seen {
            assert_eq!(refs, before[id] + 1);
        }
        assert_eq!(s.refs(), before);
    }
    assert_eq!(host.refs(), before);

    // The main canvas's walk sees only its own list.
    let mut main = Host::new(&[entry(Kind::Scene)]);
    main.scenes();
    assert_eq!(main.visited(), [0]);
}

// What the C test cannot see.

#[test]
fn enum_sources_step_order() {
    use Event::*;
    let mut host = Host::new(&[
        entry(Kind::Input),
        private(Kind::Input),
        Entry {
            refs: -1,
            ..entry(Kind::Input)
        },
        entry(Kind::Filter),
        group(),
    ]);
    host.sources();
    assert_eq!(
        host.events,
        [
            Lock,
            First(1),
            GetRef(0, true),
            Private(0),
            Input(0),
            Visit(0),
            Group(0),
            Release(0),
            Next(0),
            GetRef(1, true),
            Private(1),
            Release(1),
            Next(1),
            GetRef(2, false),
            Next(2),
            GetRef(3, true),
            Private(3),
            Input(3),
            Group(3),
            Release(3),
            Next(3),
            GetRef(4, true),
            Private(4),
            Input(4),
            Group(4),
            Visit(4),
            Release(4),
            Next(4),
            Unlock,
        ]
    );
}

#[test]
fn enum_sources_input_with_group_id() {
    // The original's `else if`: an input whose call answered true is also
    // asked for the group id, and called back again when it has it.
    let both = Entry {
        group: true,
        ..entry(Kind::Input)
    };
    let mut host = Host::new(&[both, entry(Kind::Input)]);
    host.sources();
    assert_eq!(host.visited(), [0, 0, 1]);
    assert_eq!(host.refs(), [0, 0]);

    // A false first answer stops there.
    let mut stop = Host::stopping(&[both, entry(Kind::Input)], 1);
    stop.sources();
    assert_eq!(stop.visited(), [0]);
    // A false second answer stops too.
    let mut stop = Host::stopping(&[both, entry(Kind::Input)], 2);
    stop.sources();
    assert_eq!(stop.visited(), [0, 0]);
    assert_eq!(stop.refs(), [0, 0]);
}

#[test]
fn enum_uuid_reads_next_before_each_callback() {
    use Event::*;
    let mut host = Host::new(&[entry(Kind::Input); 3]);
    host.all();
    assert_eq!(
        host.events,
        [
            Lock,
            First(1),
            Next(0),
            Visit(0),
            Next(1),
            Visit(1),
            Next(2),
            Visit(2),
            Unlock
        ]
    );

    // So a callback unlinking the current entry still reaches the rest.
    let mut unlinking = Host::new(&[entry(Kind::Input); 3]);
    unlinking.unlink_at = Some(1);
    unlinking.all();
    assert_eq!(unlinking.visited(), [0, 1, 2]);

    // A stop reads no further link.
    let mut stop = Host::stopping(&[entry(Kind::Input); 3], 2);
    stop.all();
    assert_eq!(
        stop.events,
        [Lock, First(1), Next(0), Visit(0), Next(1), Visit(1), Unlock]
    );
}

#[test]
fn enum_canvases_reads_head_before_locking() {
    use Event::*;
    let mut host = Host::new(&canvases());
    host.canvases();
    assert_eq!(
        host.events,
        [
            First(0),
            Lock,
            Next(0),
            Visit(0),
            Next(1),
            Visit(1),
            Next(2),
            Visit(2),
            Unlock
        ]
    );
    let mut unlinking = Host::new(&canvases());
    unlinking.unlink_at = Some(0);
    unlinking.canvases();
    assert_eq!(unlinking.visited(), [0, 1, 2]);
}

#[test]
fn canvas_enum_scenes_step_order() {
    use Event::*;
    let mut host = Host::new(&[
        entry(Kind::Scene),
        Entry {
            refs: -1,
            ..entry(Kind::Scene)
        },
        entry(Kind::Input),
        group(),
    ]);
    host.scenes();
    assert_eq!(
        host.events,
        [
            Lock,
            First(1),
            GetRef(0, true),
            Scene(0),
            Visit(0),
            Release(0),
            Next(0),
            GetRef(1, false),
            Next(1),
            GetRef(2, true),
            Scene(2),
            Release(2),
            Next(2),
            GetRef(3, true),
            Scene(3),
            Visit(3),
            Release(3),
            Next(3),
            Unlock,
        ]
    );
    // A stop releases, then unlocks without reading on.
    let mut stop = Host::stopping(&[entry(Kind::Scene); 2], 1);
    stop.scenes();
    assert_eq!(
        stop.events,
        [Lock, First(1), GetRef(0, true), Scene(0), Visit(0), Release(0), Unlock]
    );
}
