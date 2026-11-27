//! Tier 1 mirrors of test_source_restore_filters.c without FFI or unsafe
//! code.
use core::ffi::CStr;
use obs_source::restore_filters::{
    ARRAY_PARAMETER, RESTORE_FUNCTION, RestoreList, SOURCE_PARAMETER, restore,
};
use std::cell::{Cell, RefCell};

/// The longest chain a case starts with.
const CHAIN: usize = 6;
/// The current filters' names: an unnamed one, an empty one and a repeated
/// one (the first of the two matches).
const NAMES: [Option<&[u8]>; CHAIN] = [
    Some(b"a"),
    Some(b"bb"),
    None,
    Some(b""),
    Some(b"e"),
    Some(b"a"),
];
/// The names an array item can have: every current name, and one that no
/// current filter has.
const ITEM_NAMES: [&[u8]; 5] = [b"a", b"bb", b"", b"e", b"zz"];
/// The handle of the source itself.
const SOURCE: usize = 100;
/// No parent, no target.
const NONE: usize = 101;
/// The filter loaded for array item `k` is `LOADED + k`.
const LOADED: usize = 200;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    Lock,
    Count,
    At(usize),
    Unlink(usize),
    FreeList,
    Unlock,
    ItemCount,
    Item(usize),
    ItemName(usize),
    Name(usize),
    GetRef(usize),
    Load(usize),
    LoadPrivate(usize),
    SetTarget(usize, usize),
    SetParent(usize),
    Store(Vec<usize>),
    ReleaseItem(usize),
    Release(usize),
}

/// A model source: its list holds filter ids 0 to count - 1 (filter `i`
/// targets the next one, the last the source), named after [`NAMES`]; the
/// array's items are named as given; references and item holds are
/// counted, and every call is recorded with the lock state checked.
struct Model {
    list: RefCell<Vec<usize>>,
    targets: RefCell<Vec<usize>>,
    parents: RefCell<Vec<usize>>,
    refs: RefCell<Vec<i32>>,
    items: Vec<Option<Vec<u8>>>,
    held: RefCell<Vec<i32>>,
    dying: Option<usize>,
    locked: Cell<bool>,
    events: RefCell<Vec<Event>>,
}

/// Filter ids the model tracks: the chain's and every loaded one.
const IDS: usize = LOADED + 8;

impl Model {
    fn new(count: usize, items: &[Option<&[u8]>], dying: Option<usize>) -> Self {
        let mut targets = vec![NONE; IDS];
        let mut parents = vec![NONE; IDS];
        let mut refs = vec![0; IDS];
        for i in 0..count {
            targets[i] = if i + 1 < count { i + 1 } else { SOURCE };
            parents[i] = SOURCE;
            refs[i] = 1;
        }
        Self {
            list: RefCell::new((0..count).collect()),
            targets: RefCell::new(targets),
            parents: RefCell::new(parents),
            refs: RefCell::new(refs),
            items: items.iter().map(|n| n.map(<[u8]>::to_vec)).collect(),
            held: RefCell::new(vec![0; items.len()]),
            dying,
            locked: Cell::new(false),
            events: RefCell::new(Vec::new()),
        }
    }
    fn push(&self, event: Event) {
        self.events.borrow_mut().push(event);
    }
    fn events(&self) -> Vec<Event> {
        self.events.borrow().clone()
    }
    fn list(&self) -> Vec<usize> {
        self.list.borrow().clone()
    }
    fn unlocked(&self) {
        assert!(!self.locked.get(), "called under filter_mutex");
    }
    fn locked(&self) {
        assert!(self.locked.get(), "called without filter_mutex");
    }
}

impl RestoreList for Model {
    type Source = usize;
    type Item = usize;
    fn this(&self) -> usize {
        SOURCE
    }
    fn lock(&self) {
        assert!(!self.locked.replace(true), "locked twice");
        self.push(Event::Lock);
    }
    fn count(&self) -> usize {
        self.locked();
        self.push(Event::Count);
        self.list.borrow().len()
    }
    fn filter(&self, index: usize) -> usize {
        self.locked();
        self.push(Event::At(index));
        self.list.borrow()[index]
    }
    fn unlink(&self, filter: usize) {
        self.locked();
        self.push(Event::Unlink(filter));
        self.parents.borrow_mut()[filter] = NONE;
        self.targets.borrow_mut()[filter] = NONE;
    }
    fn free_list(&self) {
        self.locked();
        self.push(Event::FreeList);
        self.list.borrow_mut().clear();
    }
    fn unlock(&self) {
        assert!(self.locked.replace(false), "unlocked while not locked");
        self.push(Event::Unlock);
    }
    fn item_count(&self) -> usize {
        self.unlocked();
        self.push(Event::ItemCount);
        self.items.len()
    }
    fn item(&self, index: usize) -> usize {
        self.unlocked();
        self.push(Event::Item(index));
        self.held.borrow_mut()[index] += 1;
        index
    }
    fn item_name(&self, item: usize) -> Option<&[u8]> {
        self.unlocked();
        assert!(self.held.borrow()[item] > 0, "read a released item");
        self.push(Event::ItemName(item));
        self.items[item].as_deref()
    }
    fn name(&self, filter: usize) -> Option<&[u8]> {
        self.unlocked();
        self.push(Event::Name(filter));
        NAMES[filter]
    }
    fn get_ref(&self, filter: usize) -> Option<usize> {
        self.unlocked();
        self.push(Event::GetRef(filter));
        if self.dying == Some(filter) {
            return None;
        }
        self.refs.borrow_mut()[filter] += 1;
        Some(filter)
    }
    fn load(&self, item: usize) -> usize {
        self.unlocked();
        self.push(Event::Load(item));
        self.refs.borrow_mut()[LOADED + item] += 1;
        LOADED + item
    }
    fn load_private(&self, item: usize) -> usize {
        self.unlocked();
        // The list is still empty while an item loads.
        assert!(self.list.borrow().is_empty());
        self.push(Event::LoadPrivate(item));
        self.refs.borrow_mut()[LOADED + item] += 1;
        LOADED + item
    }
    fn set_target(&self, filter: usize, target: usize) {
        self.unlocked();
        self.push(Event::SetTarget(filter, target));
        self.targets.borrow_mut()[filter] = target;
    }
    fn set_parent(&self, filter: usize) {
        self.unlocked();
        self.push(Event::SetParent(filter));
        self.parents.borrow_mut()[filter] = SOURCE;
    }
    fn store(&self, filters: Vec<usize>) {
        self.locked();
        self.push(Event::Store(filters.clone()));
        *self.list.borrow_mut() = filters;
    }
    fn release_item(&self, item: usize) {
        self.unlocked();
        self.push(Event::ReleaseItem(item));
        self.held.borrow_mut()[item] -= 1;
    }
    fn release(&self, filter: usize) {
        self.unlocked();
        self.push(Event::Release(filter));
        self.refs.borrow_mut()[filter] -= 1;
    }
}

fn no_log(function: &'static CStr, parameter: &'static CStr) {
    panic!("logged {function:?}: {parameter:?}");
}

/// What the C does on a chain of `count` (with `dying` unreferenceable)
/// for the array: its calls, and the new list.
fn expected(
    count: usize,
    items: &[Option<&[u8]>],
    dying: Option<usize>,
) -> (Vec<Event>, Vec<usize>) {
    use Event::*;
    let mut events = vec![Lock, Count];
    for index in 0..count {
        events.extend([Count, At(index), Unlink(index)]);
    }
    events.extend([Count, FreeList, Unlock, ItemCount]);
    let mut list = Vec::new();
    let mut prev = None;
    for (k, item) in items.iter().enumerate() {
        events.extend([Item(k), ItemName(k)]);
        let mut found = None;
        for (j, name) in NAMES.iter().enumerate().take(count) {
            events.push(Name(j));
            if item.is_some() && name == item {
                events.push(GetRef(j));
                found = (dying != Some(j)).then_some(j);
                break;
            }
        }
        let filter = found.unwrap_or_else(|| {
            events.push(LoadPrivate(k));
            LOADED + k
        });
        if let Some(prev) = prev {
            events.push(SetTarget(prev, filter));
        }
        prev = Some(filter);
        list.push(filter);
        events.push(ReleaseItem(k));
    }
    if let Some(prev) = prev {
        events.push(SetTarget(prev, SOURCE));
    }
    events.extend(list.iter().map(|&f| SetParent(f)));
    events.extend([Lock, Store(list.clone()), Unlock]);
    events.extend((0..count).map(Release));
    (events, list)
}

/// Runs one restore and checks it against the C's calls and end state.
fn check(count: usize, items: &[Option<&[u8]>], dying: Option<usize>) -> Vec<usize> {
    let model = Model::new(count, items, dying);
    restore(Some(&model), true, no_log);
    let (events, list) = expected(count, items, dying);
    assert_eq!(model.events(), events, "{count} {items:?} {dying:?}");
    assert_eq!(model.list(), list);
    assert!(!model.locked.get());
    let targets = model.targets.borrow();
    let parents = model.parents.borrow();
    let refs = model.refs.borrow();
    // Each listed filter targets the next, the last the source; every one
    // has the source as its parent and one reference per listing.
    for (i, &filter) in list.iter().enumerate() {
        let next = list.get(i + 1).copied().unwrap_or(SOURCE);
        // A filter listed twice keeps the target written last.
        if !list[i + 1..].contains(&filter) {
            assert_eq!(targets[filter], next, "{list:?}");
        }
        assert_eq!(parents[filter], SOURCE);
        let listed = list.iter().filter(|&&f| f == filter).count();
        assert_eq!(refs[filter], i32::try_from(listed).unwrap(), "{list:?}");
    }
    // An old filter not listed again is unlinked and lost the list's
    // reference.
    for old in (0..count).filter(|f| !list.contains(f)) {
        assert_eq!((targets[old], parents[old], refs[old]), (NONE, NONE, 0));
    }
    // Every item was released as often as it was taken.
    assert!(model.held.borrow().iter().all(|&h| h == 0));
    list
}

/// Mirrors null_arguments.
#[test]
fn null_arguments_log_once_and_do_nothing() {
    let model = Model::new(3, &[Some(b"a")], None);
    let mut logs = Vec::new();

    restore(None::<&Model>, true, |f, p| logs.push((f, p)));
    restore(Some(&model), false, |f, p| logs.push((f, p)));
    // The source is checked first.
    restore(None::<&Model>, false, |f, p| logs.push((f, p)));

    assert_eq!(
        logs,
        [
            (RESTORE_FUNCTION, SOURCE_PARAMETER),
            (RESTORE_FUNCTION, ARRAY_PARAMETER),
            (RESTORE_FUNCTION, SOURCE_PARAMETER),
        ]
    );
    // Nothing of the source or the array was read, locked or written.
    assert!(model.events().is_empty());
    assert_eq!(model.list(), [0, 1, 2]);
    assert_eq!(RESTORE_FUNCTION, c"obs_source_restore_filters");
    assert_eq!(SOURCE_PARAMETER, c"source");
    assert_eq!(ARRAY_PARAMETER, c"array");
}

/// Mirrors reuses_matching_filters: a reordered, shortened array reuses
/// the filters it names, and drops the rest.
#[test]
fn reuses_matching_filters() {
    let list = check(5, &[Some(b"e"), Some(b"a"), Some(b"bb")], None);
    assert_eq!(list, [4, 0, 1]);
}

/// Mirrors loads_unmatched_items_with_the_lock_free and
/// targets_are_set_before_parents.
#[test]
fn loads_unmatched_items() {
    let list = check(
        2,
        &[Some(b"zz"), Some(b"a"), Some(b"yy"), Some(b"bb")],
        None,
    );
    assert_eq!(list, [LOADED, 0, LOADED + 2, 1]);
    // Nothing loads public: the chain rule loads private (#2981).
    let model = Model::new(0, &[Some(b"zz")], None);
    restore(Some(&model), true, no_log);
    assert!(!model.events().iter().any(|e| matches!(e, Event::Load(_))));
}

/// Mirrors empty_array_drops_every_filter.
#[test]
fn empty_array_drops_every_filter() {
    for count in 0..=CHAIN {
        assert!(check(count, &[], None).is_empty());
    }
}

/// Mirrors duplicate_names_reuse_the_same_filter and
/// unnamed_filter_never_matches; the first of two current filters of one
/// name is the one reused.
#[test]
fn duplicates_and_unnamed_filters() {
    assert_eq!(check(2, &[Some(b"a"), Some(b"a")], None), [0, 0]);
    assert_eq!(check(6, &[Some(b"a")], None), [0]);
    // Filter 2 is unnamed: an empty name matches filter 3 only.
    assert_eq!(check(4, &[Some(b""), Some(b"")], None), [3, 3]);
    // A NULL item name (never given by obs_data_get_string()) matches
    // nothing.
    assert_eq!(check(4, &[None], None), [LOADED]);
}

/// A current filter whose last reference is gone is loaded instead (not
/// reachable through the real core; Tier 3 covers it against the C).
#[test]
fn unreferenceable_filter_is_loaded() {
    assert_eq!(check(3, &[Some(b"bb"), Some(b"a")], Some(1)), [LOADED, 0]);
    // The lookup stops at the first match even when it can't be
    // referenced: the second "a" (filter 5) is not tried.
    assert_eq!(check(6, &[Some(b"a")], Some(0)), [LOADED]);
}

/// Every chain of up to 6 current filters with every array of up to 3
/// items over [`ITEM_NAMES`], with no filter or each listed one
/// unreferenceable.
#[test]
fn every_chain_and_array() {
    let mut arrays: Vec<Vec<Option<&[u8]>>> = vec![Vec::new()];
    for len in 1..=3 {
        let mut next = Vec::new();
        for array in arrays.iter().filter(|a| a.len() == len - 1) {
            for name in ITEM_NAMES {
                let mut longer = array.clone();
                longer.push(Some(name));
                next.push(longer);
            }
        }
        arrays.extend(next);
    }
    for count in 0..=CHAIN {
        for array in &arrays {
            for dying in std::iter::once(None).chain((0..count).map(Some)) {
                check(count, array, dying);
            }
        }
    }
}
