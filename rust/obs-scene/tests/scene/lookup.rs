use obs_scene::lookup::{Access, Query, find};
use std::cell::RefCell;

struct Item {
    name: Option<&'static [u8]>,
    id: i64,
    group: Option<usize>,
    next: Option<usize>,
}
struct Lists {
    first: [Option<usize>; 4],
    items: Vec<Item>,
    locks: RefCell<Vec<(bool, usize)>>,
}
impl Access for Lists {
    type Scene = usize;
    type Item = usize;
    fn with_scene<R>(&self, scene: usize, visit: impl FnOnce(Option<usize>) -> R) -> R {
        self.locks.borrow_mut().push((true, scene));
        let result = visit(self.first[scene]);
        self.locks.borrow_mut().push((false, scene));
        result
    }
    fn next(&self, item: usize) -> Option<usize> {
        self.items[item].next
    }
    fn name(&self, item: usize) -> Option<&[u8]> {
        self.items[item].name
    }
    fn id(&self, item: usize) -> i64 {
        self.items[item].id
    }
    fn is_group(&self, item: usize) -> bool {
        self.items[item].group.is_some()
    }
    fn group(&self, item: usize) -> Option<usize> {
        self.items[item].group
    }
}
fn fixture() -> Lists {
    let entry = |name, id, group, next| Item {
        name: Some(name),
        id,
        group,
        next,
    };
    Lists {
        first: [Some(0), Some(3), Some(5), None],
        items: vec![
            entry(&b"group"[..], 0, Some(1), Some(1)),
            entry(b"same", 1, None, Some(2)),
            entry(b"same", 1, None, Some(6)),
            entry(b"same", 3, None, Some(4)),
            entry(b"nested", 4, Some(2), None),
            entry(b"deep", 5, None, None),
            entry(b"", 6, Some(3), None),
        ],
        locks: RefCell::new(Vec::new()),
    }
}

#[test]
fn test_scene_lookup_characterization() {
    let mut a = fixture();
    for query in [
        Query::Name(b"same"),
        Query::RecursiveName(b"same"),
        Query::Id(1),
        Query::Group(Some(b"group")),
    ] {
        assert_eq!(find(&a, None, query), None);
    }
    assert!(a.locks.borrow().is_empty());
    assert_eq!(find(&a, Some(0), Query::Group(None)), None);
    assert_eq!(find(&a, Some(0), Query::Group(Some(b""))), None);
    assert!(a.locks.borrow().is_empty());
    assert_eq!(find(&a, Some(0), Query::Name(b"same")), Some(1));
    assert_eq!(find(&a, Some(0), Query::RecursiveName(b"same")), Some(3));
    assert_eq!(find(&a, Some(0), Query::RecursiveName(b"group")), Some(0));
    assert_eq!(find(&a, Some(0), Query::Name(b"nested")), None);
    assert_eq!(find(&a, Some(0), Query::RecursiveName(b"nested")), Some(4));
    assert_eq!(find(&a, Some(0), Query::RecursiveName(b"deep")), None);
    assert_eq!(find(&a, Some(1), Query::RecursiveName(b"deep")), Some(5));
    assert_eq!(find(&a, Some(0), Query::Id(1)), Some(1));
    assert_eq!(find(&a, Some(0), Query::Id(3)), None);
    assert_eq!(find(&a, Some(0), Query::Id(-1)), None);
    assert_eq!(find(&a, Some(0), Query::Group(Some(b"group"))), Some(0));
    assert_eq!(find(&a, Some(0), Query::Group(Some(b"same"))), None);
    assert_eq!(find(&a, Some(0), Query::Group(Some(b"nested"))), None);
    for query in [
        Query::Name(b"missing"),
        Query::RecursiveName(b"missing"),
        Query::Group(Some(b"missing")),
    ] {
        assert_eq!(find(&a, Some(0), query), None);
    }
    a.items[0].name = None;
    assert_eq!(find(&a, Some(0), Query::Group(Some(b"group"))), None);
    a.items[0].name = Some(b"group");
    a.items[0].group = None;
    a.items[2].name = Some(b"group");
    a.items[2].group = Some(1);
    assert_eq!(find(&a, Some(0), Query::Group(Some(b"group"))), Some(2));
}

#[test]
fn test_scene_canvas_characterization_and_restored_ids() {
    let mut a = fixture();
    assert_eq!(find(&a, Some(0), Query::Name(b"group")), Some(0));
    a.items[1].id = 4242;
    assert_eq!(find(&a, Some(0), Query::Id(4242)), Some(1));
    a.locks.borrow_mut().clear();
    assert_eq!(find(&a, Some(0), Query::RecursiveName(b"same")), Some(3));
    assert_eq!(
        *a.locks.borrow(),
        [(true, 0), (true, 1), (false, 1), (false, 0)]
    );
    a.first[0] = None;
    assert_eq!(find(&a, Some(0), Query::Name(b"same")), None);
    a.items[3].name = Some(b"\xff");
    assert_eq!(find(&a, Some(1), Query::Name(b"\xff")), Some(3));
}
