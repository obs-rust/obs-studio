//! Safe mirrors of test_obs_enum_contexts.c and
//! test_obs_enum_outputs_removal.c over a recording host: one kind's linked
//! context list with its re-entrant mutex, per-object reference counts, the
//! kind's get_ref/release (a release to zero destroys the object, which
//! unlinks it under the mutex, as obs_*_destroy does) and a caller callback
//! that can stop the walk, enumerate again, create an object or release its
//! own reference to the current entry. Each case runs for the three kinds.
//! Plus what the C tests cannot see: the head and every `next` are read under
//! the lock, get_ref is asked for every listed entry in list order, the
//! callback runs under the lock, and the snapshot is released in order with
//! the walk's own hold of the mutex released.
use obs_core::enum_contexts::{Contexts, enum_contexts};

const OBJECT_COUNT: usize = 3;
const KINDS: [Kind; 3] = [Kind::Output, Kind::Encoder, Kind::Service];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Output,
    Encoder,
    Service,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    Lock,
    Unlock,
    First,
    Next(usize),
    GetRef(usize, bool),
    Visit(usize),
    Release(usize),
    Destroy(usize),
}

#[derive(Debug, Clone)]
struct Object {
    refs: u32,
    next: Option<usize>,
    destroyed: bool,
}

/// One walk's callback behaviour, as the C tests' `struct walk`.
#[derive(Debug, Clone, Default)]
struct Walk {
    visited: Vec<usize>,
    /// Zero-based visit after which the callback answers false.
    stop_after: Option<usize>,
    /// On the first visit: enumerate the same kind again with this walk.
    nested: Option<Box<Walk>>,
    /// On the first visit: create one more object.
    create_in_callback: bool,
    created: Option<usize>,
    /// Zero-based visit whose callback releases the owner's reference.
    release_at: Option<usize>,
    /// Objects destroyed right after that release, inside the callback.
    dead_in_callback: Option<usize>,
}

struct Host {
    kind: Kind,
    objects: Vec<Object>,
    head: Option<usize>,
    /// The re-entrant mutex's hold count.
    depth: u32,
    events: Vec<Event>,
    /// The walks in progress, innermost last.
    walks: Vec<Walk>,
}

impl Host {
    fn new(kind: Kind) -> Self {
        Host {
            kind,
            objects: Vec::new(),
            head: None,
            depth: 0,
            events: Vec::new(),
            walks: Vec::new(),
        }
    }

    /// obs_*_create: one owner reference, inserted at the list head under
    /// the mutex (newest first).
    fn create(&mut self) -> usize {
        let id = self.objects.len();
        self.depth += 1;
        self.objects.push(Object {
            refs: 1,
            next: self.head,
            destroyed: false,
        });
        self.head = Some(id);
        self.depth -= 1;
        id
    }

    fn create_objects(&mut self) -> [usize; OBJECT_COUNT] {
        [self.create(), self.create(), self.create()]
    }

    /// The owner's obs_*_release.
    fn release_object(&mut self, id: usize) {
        let object = &mut self.objects[id];
        assert!(object.refs > 0, "release of a dead object");
        object.refs -= 1;
        if object.refs == 0 {
            self.destroy(id);
        }
    }

    /// obs_*_destroy: unlinks the object under the mutex.
    fn destroy(&mut self, id: usize) {
        self.events.push(Event::Destroy(id));
        self.depth += 1;
        let next = self.objects[id].next;
        if self.head == Some(id) {
            self.head = next;
        } else {
            let mut at = self.head;
            while let Some(entry) = at {
                if self.objects[entry].next == Some(id) {
                    self.objects[entry].next = next;
                    break;
                }
                at = self.objects[entry].next;
            }
        }
        self.objects[id].destroyed = true;
        self.depth -= 1;
    }

    fn alive(&self, id: usize) -> bool {
        !self.objects[id].destroyed
    }

    fn dead_count(&self, ids: &[usize]) -> usize {
        ids.iter().filter(|id| !self.alive(**id)).count()
    }

    /// The listed objects, head first.
    fn listed(&self) -> Vec<usize> {
        let mut out = Vec::new();
        let mut at = self.head;
        while let Some(id) = at {
            out.push(id);
            at = self.objects[id].next;
        }
        out
    }

    /// obs_enum_*() with `walk`; the mutex must be free again afterwards
    /// at the depth it started.
    fn enumerate(&mut self, walk: Walk) -> Walk {
        let depth = self.depth;
        self.walks.push(walk);
        enum_contexts(self);
        assert_eq!(self.depth, depth, "list mutex left held");
        self.walks.pop().unwrap()
    }

    fn walk(&mut self) -> Walk {
        self.enumerate(Walk::default())
    }
}

impl Contexts for Host {
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
        assert!(self.depth > 0, "head read outside the lock");
        self.events.push(Event::First);
        self.head
    }
    fn next(&mut self, entry: usize) -> Option<usize> {
        assert!(self.depth > 0, "next read outside the lock");
        assert!(self.alive(entry), "next read of a destroyed entry");
        self.events.push(Event::Next(entry));
        self.objects[entry].next
    }
    fn get_ref(&mut self, entry: usize) -> Option<usize> {
        assert!(self.depth > 0, "get_ref outside the lock");
        let object = &mut self.objects[entry];
        let got = object.refs > 0;
        if got {
            object.refs += 1;
        }
        self.events.push(Event::GetRef(entry, got));
        got.then_some(entry)
    }
    fn visit(&mut self, entry: usize) -> bool {
        assert!(self.depth > 0, "callback outside the lock");
        assert!(self.alive(entry), "callback handed a destroyed entry");
        assert!(
            self.objects[entry].refs > 0,
            "callback handed a dying entry"
        );
        self.events.push(Event::Visit(entry));
        let mut walk = self.walks.pop().expect("a walk in progress");
        let index = walk.visited.len();
        walk.visited.push(entry);
        if index == 0
            && let Some(nested) = walk.nested.take()
        {
            self.walks.push(walk);
            let nested = self.enumerate(*nested);
            walk = self.walks.pop().unwrap();
            walk.nested = Some(Box::new(nested));
        }
        if index == 0 && walk.create_in_callback {
            walk.created = Some(self.create());
        }
        if walk.release_at == Some(index) {
            self.release_object(entry);
            walk.dead_in_callback = Some(self.objects.iter().filter(|o| o.destroyed).count());
        }
        let keep_going = walk.stop_after != Some(index);
        self.walks.push(walk);
        keep_going
    }
    fn release(&mut self, entry: usize) {
        self.events.push(Event::Release(entry));
        self.release_object(entry);
    }
}

/// The newest object comes first.
fn assert_newest_first(walk: &Walk, objects: &[usize; OBJECT_COUNT], count: usize) {
    let expected: Vec<usize> = objects.iter().rev().take(count).copied().collect();
    assert_eq!(walk.visited, expected);
}

fn release_objects(host: &mut Host, objects: &[usize]) {
    for id in objects {
        host.release_object(*id);
    }
}

// test_obs_enum_contexts.c

fn check_empty(host: &mut Host) {
    host.events.clear();
    let walk = host.walk();
    assert!(walk.visited.is_empty(), "{:?}", host.kind);
}

#[test]
fn empty() {
    for kind in KINDS {
        let mut host = Host::new(kind);
        check_empty(&mut host);
        use Event::*;
        assert_eq!(host.events, [Lock, First, Unlock]);
    }
}

#[test]
fn order() {
    for kind in KINDS {
        let mut host = Host::new(kind);
        let objects = host.create_objects();
        let walk = host.walk();
        assert_newest_first(&walk, &objects, OBJECT_COUNT);
        // A second walk sees the same list: enumeration changes nothing.
        let walk = host.walk();
        assert_newest_first(&walk, &objects, OBJECT_COUNT);
        assert!(host.objects.iter().all(|o| o.refs == 1));
        release_objects(&mut host, &objects);
        check_empty(&mut host);
    }
}

#[test]
fn stop() {
    for kind in KINDS {
        let mut host = Host::new(kind);
        let objects = host.create_objects();
        for stop_after in [0, 1] {
            let walk = host.enumerate(Walk {
                stop_after: Some(stop_after),
                ..Walk::default()
            });
            assert_newest_first(&walk, &objects, stop_after + 1);
            // Every snapshot reference is dropped, visited or not.
            assert!(host.objects.iter().all(|o| o.refs == 1));
        }
        release_objects(&mut host, &objects);
    }
}

#[test]
fn nested() {
    for kind in KINDS {
        let mut host = Host::new(kind);
        let objects = host.create_objects();
        let outer = host.enumerate(Walk {
            nested: Some(Box::default()),
            ..Walk::default()
        });
        assert_newest_first(&outer, &objects, OBJECT_COUNT);
        assert_newest_first(outer.nested.as_ref().unwrap(), &objects, OBJECT_COUNT);
        assert!(host.objects.iter().all(|o| o.refs == 1));
        release_objects(&mut host, &objects);
    }
}

#[test]
fn created_in_callback() {
    for kind in KINDS {
        let mut host = Host::new(kind);
        let objects = host.create_objects();
        let walk = host.enumerate(Walk {
            create_in_callback: true,
            ..Walk::default()
        });
        let created = walk.created.expect("created in the callback");
        assert_newest_first(&walk, &objects, OBJECT_COUNT);

        // The next walk finds it, at the head of the list.
        let after = host.walk();
        assert_eq!(after.visited.len(), OBJECT_COUNT + 1);
        assert_eq!(after.visited[0], created);

        host.release_object(created);
        release_objects(&mut host, &objects);
    }
}

// test_obs_enum_outputs_removal.c

/// Walk with `target` releasing its object (or stopping the walk).
fn scenario(kind: Kind, target: usize, stop_at_target: bool) {
    let mut host = Host::new(kind);
    let objects = host.create_objects();
    let walk = host.enumerate(Walk {
        stop_after: stop_at_target.then_some(target),
        release_at: (!stop_at_target).then_some(target),
        ..Walk::default()
    });
    let released = walk.visited[target];
    if stop_at_target {
        assert_eq!(walk.visited.len(), target + 1, "early stop");
        assert_eq!(
            host.dead_count(&objects),
            0,
            "early stop destroyed an owned object"
        );
    } else {
        assert_eq!(walk.visited.len(), OBJECT_COUNT);
        // The snapshot reference keeps the object alive through its own
        // callback; the walk drops it afterwards.
        assert_eq!(
            walk.dead_in_callback,
            Some(0),
            "destroyed inside the callback"
        );
        assert_eq!(host.dead_count(&objects), 1, "snapshot reference leaked");
        assert!(!host.alive(released));
    }
    for id in objects {
        if stop_at_target || id != released {
            host.release_object(id);
        }
    }
    // Owner references are all gone, so nothing may survive the walk.
    assert_eq!(
        host.dead_count(&objects),
        OBJECT_COUNT,
        "snapshot reference leaked"
    );
    assert!(host.listed().is_empty());
}

#[test]
fn release_first() {
    for kind in KINDS {
        scenario(kind, 0, false);
    }
}

#[test]
fn release_last() {
    for kind in KINDS {
        scenario(kind, OBJECT_COUNT - 1, false);
    }
}

#[test]
fn early_stop() {
    for kind in KINDS {
        scenario(kind, 0, true);
    }
}

/// An entry still listed at refcount 0 (being destroyed) is skipped: the
/// callback is never handed it, nor NULL.
#[test]
fn skips_dying() {
    for kind in KINDS {
        let mut host = Host::new(kind);
        let keep = host.create();
        let dying = host.create();
        host.objects[dying].refs = 0;
        let walk = host.walk();
        assert_eq!(walk.visited, [keep]);
        assert_eq!(host.objects[keep].refs, 1);
        assert_eq!(host.objects[dying].refs, 0);
        assert!(
            host.alive(dying),
            "the walk released a reference it never took"
        );
        host.destroy(dying);
        host.release_object(keep);
    }
}

// What the C tests cannot see.

/// Head and every `next` are read under one hold of the lock, get_ref is
/// asked for every listed entry in list order (a dying one included), each
/// callback runs under its own hold, and the snapshot is released in order
/// after the last unlock.
#[test]
fn walk_order() {
    use Event::*;
    let mut host = Host::new(Kind::Output);
    let [a, b, c] = host.create_objects();
    host.objects[b].refs = 0;
    host.events.clear();
    let walk = host.walk();
    assert_eq!(walk.visited, [c, a]);
    assert_eq!(
        host.events,
        [
            Lock,
            First,
            GetRef(c, true),
            Next(c),
            GetRef(b, false),
            Next(b),
            GetRef(a, true),
            Next(a),
            Unlock,
            Lock,
            Visit(c),
            Unlock,
            Lock,
            Visit(a),
            Unlock,
            Release(c),
            Release(a),
        ]
    );
}

/// An early stop calls nothing more and still releases the whole snapshot,
/// in order.
#[test]
fn early_stop_releases_whole_snapshot() {
    use Event::*;
    let mut host = Host::new(Kind::Encoder);
    let [a, b, c] = host.create_objects();
    host.events.clear();
    let walk = host.enumerate(Walk {
        stop_after: Some(0),
        ..Walk::default()
    });
    assert_eq!(walk.visited, [c]);
    assert_eq!(
        host.events[9..],
        [Lock, Visit(c), Unlock, Release(c), Release(b), Release(a)]
    );
}

/// The object the callback releases is destroyed by the walk's release,
/// after the walk's last unlock.
#[test]
fn release_destroys_after_unlock() {
    use Event::*;
    let mut host = Host::new(Kind::Service);
    let [a, b, c] = host.create_objects();
    host.events.clear();
    let walk = host.enumerate(Walk {
        release_at: Some(1),
        ..Walk::default()
    });
    assert_eq!(walk.visited, [c, b, a]);
    assert_eq!(
        host.events[9..],
        [
            Lock,
            Visit(c),
            Unlock,
            Lock,
            Visit(b),
            Unlock,
            Lock,
            Visit(a),
            Unlock,
            Release(c),
            Release(b),
            Destroy(b),
            Release(a),
        ]
    );
    assert_eq!(host.listed(), [c, a]);
}

/// A nested walk takes and drops its own holds inside the outer callback's,
/// and releases its snapshot before the outer walk goes on.
#[test]
fn nested_walk_inside_callback_hold() {
    use Event::*;
    let mut host = Host::new(Kind::Output);
    let a = host.create();
    host.events.clear();
    host.enumerate(Walk {
        nested: Some(Box::default()),
        ..Walk::default()
    });
    assert_eq!(
        host.events,
        [
            Lock,
            First,
            GetRef(a, true),
            Next(a),
            Unlock,
            Lock,
            Visit(a),
            Lock,
            First,
            GetRef(a, true),
            Next(a),
            Unlock,
            Lock,
            Visit(a),
            Unlock,
            Release(a),
            Unlock,
            Release(a),
        ]
    );
    assert_eq!(host.objects[a].refs, 1);
}
