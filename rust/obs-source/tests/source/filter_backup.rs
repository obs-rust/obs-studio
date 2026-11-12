//! Tier 1 mirrors test_source_filter_backup.c through the safe backup API.
use obs_source::filter_backup::{Backend, backup};
use std::rc::{Rc, Weak};

#[derive(Clone, Debug, PartialEq)]
struct Record {
    name: String,
    id: String,
    versioned_id: String,
    uuid: String,
    enabled: bool,
    marker: usize,
    ordinal: usize,
}

#[derive(Default)]
struct Source {
    filters: Vec<Record>,
    locked: bool,
    order: String,
    events: Vec<&'static str>,
    saved: Vec<Weak<Record>>,
    grow_on_save: bool,
}

fn record(index: usize) -> Record {
    Record {
        name: ["alpha", "beta", "gamma"][index].into(),
        id: "backup_filter".into(),
        versioned_id: "backup_filter".into(),
        uuid: format!("uuid-{index}"),
        enabled: index != 0,
        marker: 10 + index,
        ordinal: 0,
    }
}

impl Backend for Source {
    type Array = Vec<Rc<Record>>;
    type Saved = Rc<Record>;

    fn create_array(&mut self) -> Self::Array {
        assert!(!self.locked);
        self.events.push("create");
        Vec::new()
    }
    fn lock(&mut self) {
        assert!(!self.locked);
        self.locked = true;
        self.events.push("lock");
    }
    fn count(&self) -> usize {
        assert!(self.locked);
        self.filters.len()
    }
    fn save(&mut self, index: usize) -> Self::Saved {
        assert!(self.locked);
        self.events.push("save");
        let mut saved = self.filters[index].clone();
        self.order.push(saved.name.chars().next().unwrap());
        saved.ordinal = self.order.len();
        let saved = Rc::new(saved);
        self.saved.push(Rc::downgrade(&saved));
        if self.grow_on_save {
            self.grow_on_save = false;
            self.filters.push(record(1));
        }
        saved
    }
    fn push(&mut self, array: &mut Self::Array, saved: &Self::Saved) {
        assert!(self.locked);
        self.events.push("push");
        array.push(Rc::clone(saved));
    }
    fn release(&mut self, saved: Self::Saved) {
        assert!(self.locked);
        self.events.push("release");
        assert_eq!(Rc::strong_count(&saved), 2);
        drop(saved);
    }
    fn unlock(&mut self) {
        assert!(self.locked);
        self.events.push("unlock");
        self.locked = false;
    }
}

fn no_log(_: &str) {
    panic!("a live source must not log");
}

#[test]
fn test_source_filter_backup_null_source_is_rejected() {
    let mut messages = Vec::new();
    assert!(backup::<Source>(None, |message| messages.push(message)).is_none());
    assert_eq!(messages, ["obs_source_backup_filters: Null 'source' parameter"]);
}

#[test]
fn test_source_filter_backup_empty_backups_are_owned_arrays() {
    let mut source = Source::default();
    let mut first = backup(Some(&mut source), no_log).unwrap();
    let second = backup(Some(&mut source), no_log).unwrap();
    assert!(first.is_empty());
    assert!(second.is_empty());
    first.push(Rc::new(record(0)));
    assert_eq!(first.len(), 1);
    assert!(second.is_empty());
    assert!(source.filters.is_empty());
    assert!(!source.locked);
    assert_eq!(source.events, ["create", "lock", "unlock", "create", "lock", "unlock"]);
}

fn check_populated_backup(count: usize) {
    let mut source = Source {
        filters: (0..count).rev().map(record).collect(),
        ..Source::default()
    };
    let array = backup(Some(&mut source), no_log).unwrap();
    assert_eq!(array.len(), count);
    assert_eq!(source.order, if count == 1 { "a" } else { "gba" });
    assert_eq!(source.saved.len(), count);
    assert!(!source.locked);
    let held = Rc::clone(&array[0]);
    for (i, entry) in array.iter().enumerate() {
        let index = count - i - 1;
        let mut expected = record(index);
        assert_eq!(source.filters[i], expected);
        expected.ordinal = i + 1;
        assert_eq!(**entry, expected);
    }
    let mut expected_events = vec!["create", "lock"];
    for _ in 0..count {
        expected_events.extend(["save", "push", "release"]);
    }
    expected_events.push("unlock");
    assert_eq!(source.events, expected_events);
    drop(source);
    assert_eq!(array.last().unwrap().name, "alpha");
    drop(array);
    assert_eq!(held.name, record(count - 1).name);
    assert_eq!(held.marker, 10 + count - 1);
}

#[test]
fn test_source_filter_backup_singleton_backup() {
    check_populated_backup(1);
}

#[test]
fn test_source_filter_backup_forward_order_and_ownership() {
    check_populated_backup(3);
}

#[test]
fn test_source_filter_backup_releases_temporary_references() {
    let mut source = Source { filters: vec![record(0)], ..Source::default() };
    for _ in 0..2 {
        let array = backup(Some(&mut source), no_log).unwrap();
        assert_eq!(array.len(), 1);
        assert_eq!(Rc::strong_count(&array[0]), 1);
        drop(array);
        assert!(source.saved.iter().all(|saved| saved.upgrade().is_none()));
        assert!(!source.locked);
    }
}

#[test]
fn save_callback_can_extend_the_recursive_locked_list() {
    let mut source = Source {
        filters: vec![record(0)],
        grow_on_save: true,
        ..Source::default()
    };
    let array = backup(Some(&mut source), no_log).unwrap();
    assert_eq!(source.order, "ab");
    assert_eq!(array.len(), 2);
    assert!(!source.locked);
}
