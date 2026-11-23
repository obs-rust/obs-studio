//! Safe mirrors of test_reset_source_uuids.c over a recording host: the
//! core's uuid table as uthash keeps it (each entry's next and previous link
//! in its table, insertion order, the head moving on when its entry is
//! unlinked, an unlinked entry keeping its own next link and an added one
//! appended at the tail with no next link), a sources mutex, each entry's
//! name and uuid, and a uuid generator that hands out well formed, never
//! repeated uuids. Every case of the C test is mirrored on the C test's five
//! sources (an input, a private input, a scene, an input and a private
//! scene, in creation order).
//!
//! Plus what the C test cannot see: the exact order of every read and call
//! (the head read once under the lock, each next link read before its entry
//! is unlinked, then the free, the generation and the add, the head stored
//! once after the walk, the mutex held throughout), the empty table storing
//! no head, a free of each old uuid exactly once and the working table
//! emptied by the walk.
use obs_core::reset_source_uuids::{Sources, reset_source_uuids};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    Lock,
    Unlock,
    Head,
    Next(usize),
    Delete(usize),
    Free(usize),
    Generate(usize),
    Add(usize),
    Store(Option<usize>),
    /// A step other than the lock taken without the mutex held.
    Unlocked,
    /// A delete from a table the entry is not in.
    WrongTable(usize),
    /// An add of an entry still in a table, or without a uuid.
    BadAdd(usize),
    /// A free of an already freed uuid.
    DoubleFree(usize),
}

/// The C test's populate(), in creation order.
const NAMES: [&str; 5] = [
    "input a",
    "private input",
    "scene",
    "input b",
    "private scene",
];

struct Host {
    names: Vec<&'static str>,
    uuids: Vec<Option<String>>,
    next: Vec<Option<usize>>,
    prev: Vec<Option<usize>>,
    /// The table each entry is listed in.
    table_of: Vec<Option<usize>>,
    /// Each table's entries in insertion order.
    tables: Vec<Vec<usize>>,
    /// The core's head.
    core: Option<usize>,
    depth: u32,
    generated: u64,
    events: Vec<Event>,
    freed: Vec<String>,
}

impl Host {
    fn new(names: &[&'static str]) -> Self {
        let count = names.len();
        let mut host = Host {
            names: names.to_vec(),
            uuids: vec![None; count],
            next: vec![None; count],
            prev: vec![None; count],
            table_of: vec![None; count],
            tables: Vec::new(),
            core: None,
            depth: 0,
            generated: 0,
            events: Vec::new(),
            freed: Vec::new(),
        };
        // obs_context_data_init_wrap: a uuid, then the add to the table.
        let mut core = None;
        for i in 0..count {
            host.uuids[i] = Some(host.fresh());
            host.link(&mut core, i);
        }
        host.core = core;
        host
    }

    fn populated() -> Self {
        Host::new(&NAMES)
    }

    /// A well formed uuid never handed out before.
    fn fresh(&mut self) -> String {
        self.generated += 1;
        let n = self.generated;
        format!(
            "{:08x}-{:04x}-4{:03x}-8000-{:012x}",
            n,
            n & 0xffff,
            n & 0xfff,
            n
        )
    }

    fn link(&mut self, table: &mut Option<usize>, entry: usize) {
        self.next[entry] = None;
        match *table {
            None => {
                self.prev[entry] = None;
                self.table_of[entry] = Some(self.tables.len());
                self.tables.push(vec![entry]);
                *table = Some(entry);
            }
            Some(head) => {
                let id = self.table_of[head].unwrap();
                let tail = *self.tables[id].last().unwrap();
                self.next[tail] = Some(entry);
                self.prev[entry] = Some(tail);
                self.table_of[entry] = Some(id);
                self.tables[id].push(entry);
            }
        }
    }

    fn check_locked(&mut self) {
        if self.depth == 0 {
            self.events.push(Event::Unlocked);
        }
    }

    fn uuid(&self, entry: usize) -> &str {
        self.uuids[entry].as_deref().unwrap()
    }

    fn all_uuids(&self) -> Vec<String> {
        (0..self.names.len())
            .map(|i| self.uuid(i).to_owned())
            .collect()
    }

    /// obs_get_source_by_uuid's table lookup.
    fn find(&self, uuid: &str) -> Option<usize> {
        let id = self.table_of[self.core?]?;
        self.tables[id]
            .iter()
            .copied()
            .find(|&e| self.uuids[e].as_deref() == Some(uuid))
    }

    /// obs_enum_all_sources: the core's table by its next links.
    fn walk(&self) -> Vec<usize> {
        let mut seen = Vec::new();
        let mut entry = self.core;
        while let Some(e) = entry {
            seen.push(e);
            entry = self.next[e];
        }
        seen
    }

    fn reset(&mut self) {
        self.events.clear();
        reset_source_uuids(self);
        assert_eq!(self.depth, 0, "the mutex is left held");
    }
}

impl Sources for Host {
    type Entry = usize;
    fn lock(&mut self) {
        self.depth += 1;
        self.events.push(Event::Lock);
    }
    fn unlock(&mut self) {
        self.check_locked();
        self.depth = self.depth.saturating_sub(1);
        self.events.push(Event::Unlock);
    }
    fn head(&mut self) -> Option<usize> {
        self.check_locked();
        self.events.push(Event::Head);
        self.core
    }
    fn next(&mut self, entry: usize) -> Option<usize> {
        self.check_locked();
        self.events.push(Event::Next(entry));
        self.next[entry]
    }
    fn delete(&mut self, table: &mut Option<usize>, entry: usize) {
        self.check_locked();
        self.events.push(Event::Delete(entry));
        let id = self.table_of[entry];
        if id.is_none() || table.and_then(|h| self.table_of[h]) != id {
            self.events.push(Event::WrongTable(entry));
            return;
        }
        let id = id.unwrap();
        if let Some(p) = self.prev[entry] {
            self.next[p] = self.next[entry];
        }
        if let Some(n) = self.next[entry] {
            self.prev[n] = self.prev[entry];
        }
        self.tables[id].retain(|&e| e != entry);
        self.table_of[entry] = None;
        // uthash leaves the unlinked entry's own next link as it was.
        *table = self.tables[id].first().copied();
    }
    fn free_uuid(&mut self, entry: usize) {
        self.check_locked();
        self.events.push(Event::Free(entry));
        match self.uuids[entry].take() {
            Some(old) => self.freed.push(old),
            None => self.events.push(Event::DoubleFree(entry)),
        }
    }
    fn new_uuid(&mut self, entry: usize) {
        self.check_locked();
        self.events.push(Event::Generate(entry));
        let uuid = self.fresh();
        self.uuids[entry] = Some(uuid);
    }
    fn add(&mut self, table: &mut Option<usize>, entry: usize) {
        self.check_locked();
        self.events.push(Event::Add(entry));
        if self.table_of[entry].is_some() || self.uuids[entry].is_none() {
            self.events.push(Event::BadAdd(entry));
            return;
        }
        self.link(table, entry);
    }
    fn store(&mut self, table: Option<usize>) {
        self.check_locked();
        self.events.push(Event::Store(table));
        self.core = table;
    }
}

fn well_formed_uuid(uuid: &str) -> bool {
    uuid.len() == 36
        && uuid.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        })
}

fn assert_all_new_and_unique(host: &Host, old: &[String]) {
    let count = host.names.len();
    for i in 0..count {
        let uuid = host.uuid(i);
        assert!(well_formed_uuid(uuid), "{uuid}");
        assert!(!old.iter().any(|o| o == uuid), "{i} kept an old uuid");
        for j in 0..count {
            if j != i {
                assert_ne!(uuid, host.uuid(j), "{i} and {j} share a uuid");
            }
        }
    }
}

#[test]
fn every_source_gets_a_new_unique_uuid() {
    let mut host = Host::populated();
    let old = host.all_uuids();

    host.reset();

    assert_all_new_and_unique(&host, &old);
    // Each old uuid freed exactly once.
    assert_eq!(host.freed, old);
}

#[test]
fn lookup_by_new_uuid_finds_the_source_old_finds_nothing() {
    let mut host = Host::populated();
    let old = host.all_uuids();

    host.reset();

    for (i, old) in old.iter().enumerate() {
        assert_eq!(host.find(host.uuid(i)), Some(i));
        assert_eq!(host.find(old), None);
    }
}

#[test]
fn names_and_order_are_kept() {
    assert!(Host::new(&[]).walk().is_empty());

    let mut host = Host::populated();
    let order: Vec<usize> = (0..NAMES.len()).collect();
    assert_eq!(host.walk(), order);

    host.reset();

    assert_eq!(host.walk(), order);
    assert_eq!(host.names, NAMES);
    // Every source is still found, under its new uuid, by name too.
    for (i, name) in NAMES.iter().enumerate() {
        let found = host.walk().into_iter().find(|&e| host.names[e] == *name);
        assert_eq!(found, Some(i));
    }
}

#[test]
fn no_sources_is_a_no_op() {
    let mut host = Host::new(&[]);

    host.reset();

    assert!(host.walk().is_empty());
    assert_eq!(
        host.events,
        [Event::Lock, Event::Head, Event::Store(None), Event::Unlock]
    );
    assert_eq!(host.generated, 0);
}

#[test]
fn second_reset_changes_uuids_again() {
    let mut host = Host::populated();
    let first = host.all_uuids();

    host.reset();
    let second = host.all_uuids();
    host.reset();

    assert_all_new_and_unique(&host, &second);
    assert_all_new_and_unique(&host, &first);
    for (i, old) in second.iter().enumerate() {
        assert_eq!(host.find(host.uuid(i)), Some(i));
        assert_eq!(host.find(old), None);
    }
    assert_eq!(host.walk(), (0..NAMES.len()).collect::<Vec<_>>());
}

/// The original's order: the head once under the lock; per entry, its next
/// link before the unlink, the free, the generation and the add; the head
/// stored once after the walk, then the unlock.
#[test]
fn every_read_and_call_in_the_original_order() {
    let mut host = Host::new(&NAMES[..3]);

    host.reset();

    let mut expected = vec![Event::Lock, Event::Head];
    for e in 0..3 {
        expected.extend([
            Event::Next(e),
            Event::Delete(e),
            Event::Free(e),
            Event::Generate(e),
            Event::Add(e),
        ]);
    }
    expected.extend([Event::Store(Some(0)), Event::Unlock]);
    assert_eq!(host.events, expected);
}

#[test]
fn one_source_moves_to_a_new_table() {
    let mut host = Host::new(&NAMES[..1]);
    let old = host.all_uuids();
    let old_table = host.table_of[0];

    host.reset();

    assert_eq!(
        host.events,
        [
            Event::Lock,
            Event::Head,
            Event::Next(0),
            Event::Delete(0),
            Event::Free(0),
            Event::Generate(0),
            Event::Add(0),
            Event::Store(Some(0)),
            Event::Unlock,
        ]
    );
    assert_ne!(host.table_of[0], old_table);
    assert!(host.tables[old_table.unwrap()].is_empty());
    assert_eq!(host.freed, old);
    assert_eq!(host.walk(), [0]);
}

/// The working table is emptied entry by entry from its head, the new one
/// filled in the same order, and the core keeps the old head until the
/// single store.
#[test]
fn the_old_table_empties_and_the_head_is_stored_once() {
    let mut host = Host::populated();
    let old_table = host.table_of[0].unwrap();

    host.reset();

    assert!(host.tables[old_table].is_empty());
    let stores: Vec<&Event> = host
        .events
        .iter()
        .filter(|e| matches!(e, Event::Store(_)))
        .collect();
    assert_eq!(stores, [&Event::Store(Some(0))]);
    assert_eq!(
        host.events[host.events.len() - 2..],
        [Event::Store(Some(0)), Event::Unlock]
    );
    assert!(!host.events.iter().any(|e| matches!(
        e,
        Event::Unlocked | Event::WrongTable(_) | Event::BadAdd(_) | Event::DoubleFree(_)
    )));
    let new_table = host.table_of[0].unwrap();
    assert_eq!(host.tables[new_table], (0..NAMES.len()).collect::<Vec<_>>());
}
