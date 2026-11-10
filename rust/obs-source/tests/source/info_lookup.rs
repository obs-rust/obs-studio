//! Tier 1: the source type lookup over a model registry. The cases follow
//! test/cmocka/test_source_info_lookup.c one to one, then the NULL IDs.
use obs_source::info_lookup::{Registry, find};

/// A registry of IDs (`None` is a NULL ID) that logs every read.
struct Model {
    ids: Vec<Option<Vec<u8>>>,
    reads: Vec<Read>,
}

#[derive(Debug, PartialEq)]
enum Read {
    Count,
    Id(usize),
}

impl Model {
    fn new(ids: &[Option<&str>]) -> Self {
        Self {
            ids: ids
                .iter()
                .map(|id| id.map(|id| id.as_bytes().to_vec()))
                .collect(),
            reads: Vec::new(),
        }
    }
}

impl Registry for Model {
    fn count(&mut self) -> usize {
        self.reads.push(Read::Count);
        self.ids.len()
    }

    fn id(&mut self, index: usize) -> Option<&[u8]> {
        self.reads.push(Read::Id(index));
        self.ids[index].as_deref()
    }
}

fn lookup(ids: &[Option<&str>], id: &str) -> Option<usize> {
    find(&mut Model::new(ids), Some(id.as_bytes()))
}

#[test]
fn unknown_ids_before_registration() {
    assert_eq!(lookup(&[], "lookup_first"), None);
    assert_eq!(lookup(&[], ""), None);
}

#[test]
fn registered_ids_and_near_misses() {
    let ids = [Some("lookup_first"), Some("lookup_second")];
    assert_eq!(lookup(&ids, "lookup_first"), Some(0));
    assert_eq!(lookup(&ids, "lookup_second"), Some(1));
    for miss in [
        "",
        "lookup",
        "lookup_firs",
        "lookup_first ",
        "lookup_firstx",
        "LOOKUP_FIRST",
        "Lookup_first",
        "lookup_third",
    ] {
        assert_eq!(lookup(&ids, miss), None, "{miss:?}");
    }
}

#[test]
fn first_registered_match_wins() {
    let ids = [Some("lookup_dup_v1"), Some("lookup_dup_v1")];
    assert_eq!(lookup(&ids, "lookup_dup_v1"), Some(0));
    assert_eq!(lookup(&ids, "lookup_dup"), None);
}

#[test]
fn empty_ids_and_bytes_are_compared_whole() {
    // An empty registered ID matches only the empty ID.
    let ids = [Some("a"), Some(""), Some("b")];
    assert_eq!(lookup(&ids, ""), Some(1));
    assert_eq!(lookup(&ids, "b"), Some(2));
    // Bytes, not characters: no folding, no normalization.
    let mut model = Model {
        ids: vec![Some(b"\xc3\xa9".to_vec()), Some(b"\xff".to_vec())],
        reads: Vec::new(),
    };
    assert_eq!(find(&mut model, Some(b"\xff")), Some(1));
    assert_eq!(find(&mut model, Some("é".as_bytes())), Some(0));
    assert_eq!(find(&mut model, Some(b"e\xcc\x81")), None);
}

#[test]
fn reads_follow_the_c_loop() {
    // The count is read before every entry and once more at the end.
    let mut model = Model::new(&[Some("a"), Some("b")]);
    assert_eq!(find(&mut model, Some(b"c")), None);
    assert_eq!(
        model.reads,
        [
            Read::Count,
            Read::Id(0),
            Read::Count,
            Read::Id(1),
            Read::Count
        ]
    );
    // A match stops the walk.
    let mut model = Model::new(&[Some("a"), Some("b")]);
    assert_eq!(find(&mut model, Some(b"a")), Some(0));
    assert_eq!(model.reads, [Read::Count, Read::Id(0)]);
}

/// Intentional difference: the C crashes in strcmp on a NULL ID once any type
/// is registered; the port finds nothing and reads nothing.
#[test]
fn null_id_finds_nothing() {
    for ids in [&[][..], &[Some("a")][..], &[None, Some("")][..]] {
        let mut model = Model::new(ids);
        assert_eq!(find(&mut model, None), None);
        assert!(model.reads.is_empty());
    }
}

/// Intentional difference: a registered NULL ID matches nothing, where the C
/// would pass it to strcmp.
#[test]
fn null_registered_id_matches_nothing() {
    let ids = [None, Some("a")];
    assert_eq!(lookup(&ids, "a"), Some(1));
    assert_eq!(lookup(&ids, ""), None);
}
