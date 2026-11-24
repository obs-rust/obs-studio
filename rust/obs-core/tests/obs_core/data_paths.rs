//! Safe mirrors of test_data_paths.c (#5355) and of the data path steps of
//! test_core_setup_rules_steps.c over a model of the C list: each entry the
//! bytes `dstr_init_copy()` keeps (none for NULL or ""), in order. Every case
//! of the C test that needs no core is mirrored; its lifecycle case (the list
//! outlives startup and shutdown) holds by construction here, since the core
//! takes no part, and stays with the unchanged C test.
//!
//! Plus what the C test cannot see: the calls made (one push per add, a
//! count read before each comparison, the one erase at the first equal
//! index, no erase on a miss), and `compare()`'s sign against `strcmp()`.
use std::cmp::Ordering;

use obs_core::data_paths::{PathList, add, compare, remove};
use proptest::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Call {
    Count,
    Push,
    Erase(usize),
}

#[derive(Debug, Default)]
struct List {
    entries: Vec<Vec<u8>>,
    calls: std::cell::RefCell<Vec<Call>>,
}

impl PathList for List {
    type Path = Option<&'static [u8]>;
    fn count(&self) -> usize {
        self.calls.borrow_mut().push(Call::Count);
        self.entries.len()
    }
    fn entry(&self, index: usize) -> &[u8] {
        &self.entries[index]
    }
    fn push_copy(&mut self, path: Option<&'static [u8]>) {
        self.calls.get_mut().push(Call::Push);
        self.entries.push(path.unwrap_or_default().to_vec());
    }
    fn erase(&mut self, index: usize) {
        self.calls.get_mut().push(Call::Erase(index));
        self.entries.remove(index);
    }
}

impl List {
    fn add(&mut self, path: Option<&'static str>) {
        add(self, path.map(str::as_bytes));
    }
    fn remove(&mut self, path: Option<&str>) -> bool {
        remove(self, path.unwrap_or_default().as_bytes())
    }
    fn names(&self) -> Vec<&str> {
        self.entries
            .iter()
            .map(|entry| std::str::from_utf8(entry).unwrap())
            .collect()
    }
    fn take_calls(&mut self) -> Vec<Call> {
        std::mem::take(self.calls.get_mut())
    }
}

#[test]
fn add_remove_without_core() {
    let mut list = List::default();
    assert!(!list.remove(Some("test_data_paths_never/")));
    list.add(Some("test_data_paths_x/"));
    assert!(list.remove(Some("test_data_paths_x/")));
    assert!(!list.remove(Some("test_data_paths_x/")));
    assert!(list.entries.is_empty());
}

#[test]
fn duplicates_need_one_remove_each() {
    let mut list = List::default();
    for _ in 0..3 {
        list.add(Some("test_data_paths_dup/"));
    }
    for _ in 0..3 {
        assert!(list.remove(Some("test_data_paths_dup/")));
    }
    assert!(!list.remove(Some("test_data_paths_dup/")));
}

#[test]
fn match_is_exact() {
    let mut list = List::default();
    list.add(Some("test_data_paths_Case/"));
    for other in [
        "test_data_paths_case/",
        "TEST_DATA_PATHS_CASE/",
        "test_data_paths_Case",
        "test_data_paths_Case//",
        "./test_data_paths_Case/",
        "test_data_paths_Cas",
    ] {
        assert!(!list.remove(Some(other)), "{other}");
    }
    assert!(list.remove(Some("test_data_paths_Case/")));
    assert!(!list.remove(Some("test_data_paths_Case/")));
}

#[test]
fn null_and_empty_are_the_same_path() {
    let mut list = List::default();
    assert!(!list.remove(None));
    assert!(!list.remove(Some("")));

    list.add(None);
    assert!(list.remove(Some("")));
    assert!(!list.remove(None));

    list.add(Some(""));
    assert!(list.remove(None));
    assert!(!list.remove(Some("")));

    list.add(None);
    list.add(Some(""));
    assert!(list.remove(None));
    assert!(list.remove(None));
    assert!(!list.remove(Some("")));

    list.add(Some(""));
    assert!(!list.remove(Some("test_data_paths_x/")));
    list.add(Some("test_data_paths_x/"));
    assert!(list.remove(Some("")));
    assert!(!list.remove(None));
    assert!(list.remove(Some("test_data_paths_x/")));
}

/// test_remove_keeps_order_of_the_rest and the setup-rules steps: the first
/// equal entry goes, the rest keep their order (which `obs_find_data_file()`
/// searches).
#[test]
fn remove_keeps_order_of_the_rest() {
    let mut list = List::default();
    list.add(Some("a/"));
    list.add(Some("b/"));
    list.add(Some("a/"));
    assert!(!list.remove(Some("c/")));
    assert_eq!(list.names(), ["a/", "b/", "a/"]);
    assert!(list.remove(Some("a/")));
    assert_eq!(list.names(), ["b/", "a/"]);
    assert!(list.remove(Some("a/")));
    assert_eq!(list.names(), ["b/"]);
    assert!(!list.remove(Some("a/")));
    assert!(list.remove(Some("b/")));
    assert!(list.names().is_empty());
}

#[test]
fn calls_made() {
    let mut list = List::default();
    list.add(Some("a/"));
    assert_eq!(list.take_calls(), [Call::Push]);
    list.add(Some("b/"));
    list.add(Some("c/"));
    list.take_calls();

    assert!(list.remove(Some("b/")));
    assert_eq!(
        list.take_calls(),
        [Call::Count, Call::Count, Call::Erase(1)]
    );
    assert!(!list.remove(Some("x/")));
    assert_eq!(list.take_calls(), [Call::Count, Call::Count, Call::Count]);
    assert!(list.remove(Some("a/")));
    assert_eq!(list.take_calls(), [Call::Count, Call::Erase(0)]);
    list.remove(Some("c/"));
    list.take_calls();
    assert!(!list.remove(Some("c/")));
    assert_eq!(list.take_calls(), [Call::Count]);
}

fn strcmp_sign(a: &[u8], b: &[u8]) -> i32 {
    // strcmp() over the terminated strings: the terminator is below every
    // other byte, so the shorter prefix sorts first.
    match a.cmp(b) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

#[test]
fn compare_signs() {
    assert_eq!(compare(b"", b""), 0);
    assert_eq!(compare(b"a", b""), 1);
    assert_eq!(compare(b"", b"a"), -1);
    assert_eq!(compare(b"ab", b"a"), 1);
    assert_eq!(compare(b"a", b"b"), -1);
    assert_eq!(compare(b"\xff", b"a"), 1, "bytes compare unsigned");
    assert_eq!(compare(b"A", b"a"), -1);
}

proptest! {
    #[test]
    fn compare_is_strcmp(
        a in proptest::collection::vec(1u8.., 0..8),
        b in proptest::collection::vec(1u8.., 0..8),
    ) {
        prop_assert_eq!(compare(&a, &b), strcmp_sign(&a, &b));
        prop_assert_eq!(compare(&a, &a), 0);
    }

    /// Any run of adds and removes over a few names leaves the model a
    /// plain first-match list would.
    #[test]
    fn matches_a_first_match_list(ops in proptest::collection::vec((any::<bool>(), 0usize..4), 0..24)) {
        const NAMES: [Option<&str>; 4] = [None, Some(""), Some("a/"), Some("b/")];
        let mut list = List::default();
        let mut model: Vec<&str> = Vec::new();
        for (is_add, name) in ops {
            let path = NAMES[name];
            let key = path.unwrap_or_default();
            if is_add {
                list.add(path);
                model.push(key);
            } else {
                let at = model.iter().position(|entry| *entry == key);
                prop_assert_eq!(list.remove(path), at.is_some());
                if let Some(at) = at {
                    model.remove(at);
                }
            }
            prop_assert_eq!(list.names(), model.clone());
        }
    }
}
