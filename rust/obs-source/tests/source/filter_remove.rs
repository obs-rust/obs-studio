//! Tier 1 mirrors of test_source_filter_remove.c without FFI or unsafe code.
use core::ffi::CStr;
use obs_source::filter_remove::{
    FILTER_PARAMETER, FilterList, REMOVE_FUNCTION, SOURCE_PARAMETER, remove, remove_refless,
};
use std::cell::{Cell, RefCell};

/// The most filters a chain gets here.
const FILTERS: usize = 7;
/// The handle of the source itself, what the last filter targets.
const SOURCE: usize = 100;
/// The handle of NULL, a cleared target.
const NULL: usize = 101;
/// A filter on no list.
const OUTSIDER: usize = 99;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Lock,
    Count,
    At(usize),
    Target(usize),
    SetTarget(usize, usize),
    Erase(usize),
    Unlock,
    Signal(usize),
    Log(usize),
    HasCallback(usize),
    Callback(usize),
    Clear(usize),
    Release(usize),
}

/// A model source: its list holds filter ids (filter `i` starts targeting
/// the next one, the last the source), filter `i` has a `filter_remove`
/// callback when bit `i` of the mask is set, and every call is recorded.
struct Model {
    list: RefCell<Vec<usize>>,
    targets: RefCell<Vec<usize>>,
    parents: RefCell<Vec<bool>>,
    callbacks: u32,
    locked: Cell<bool>,
    events: RefCell<Vec<Event>>,
}

impl Model {
    fn new(count: usize, callbacks: u32) -> Self {
        let mut targets: Vec<usize> = (1..=FILTERS).collect();
        if count > 0 {
            targets[count - 1] = SOURCE;
        }
        Self {
            list: RefCell::new((0..count).collect()),
            targets: RefCell::new(targets),
            parents: RefCell::new((0..FILTERS).map(|i| i < count).collect()),
            callbacks,
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
    /// Each listed filter's target, in list order.
    fn chain(&self) -> Vec<(usize, usize)> {
        let targets = self.targets.borrow();
        self.list().into_iter().map(|f| (f, targets[f])).collect()
    }
}

impl FilterList for Model {
    type Source = usize;
    fn lock(&self) {
        assert!(!self.locked.replace(true), "locked twice");
        self.push(Event::Lock);
    }
    fn count(&self) -> usize {
        assert!(self.locked.get());
        self.push(Event::Count);
        self.list.borrow().len()
    }
    fn filter(&self, index: usize) -> usize {
        assert!(self.locked.get());
        self.push(Event::At(index));
        self.list.borrow()[index]
    }
    fn target(&self, filter: usize) -> usize {
        assert!(self.locked.get());
        self.push(Event::Target(filter));
        self.targets.borrow()[filter]
    }
    fn set_target(&self, filter: usize, target: usize) {
        assert!(self.locked.get());
        self.push(Event::SetTarget(filter, target));
        self.targets.borrow_mut()[filter] = target;
    }
    fn erase(&self, index: usize) {
        assert!(self.locked.get());
        self.push(Event::Erase(index));
        self.list.borrow_mut().remove(index);
    }
    fn unlock(&self) {
        assert!(self.locked.replace(false), "unlocked while not locked");
        self.push(Event::Unlock);
    }
    fn signal(&self, filter: usize) {
        assert!(!self.locked.get(), "signalled under the lock");
        // The list no longer holds the filter; its parent is still set.
        assert!(!self.list.borrow().contains(&filter));
        assert!(self.parents.borrow()[filter]);
        self.push(Event::Signal(filter));
    }
    fn log_removed(&self, filter: usize) {
        assert!(!self.locked.get());
        self.push(Event::Log(filter));
    }
    fn has_remove_callback(&self, filter: usize) -> bool {
        assert!(!self.locked.get());
        self.push(Event::HasCallback(filter));
        self.callbacks & (1 << filter) != 0
    }
    fn call_remove_callback(&self, filter: usize) {
        assert!(!self.locked.get());
        assert!(
            self.parents.borrow()[filter],
            "callback after the parent was cleared"
        );
        self.push(Event::Callback(filter));
    }
    fn clear_links(&self, filter: usize) {
        assert!(!self.locked.get());
        self.push(Event::Clear(filter));
        self.parents.borrow_mut()[filter] = false;
        self.targets.borrow_mut()[filter] = NULL;
    }
    fn release(&self, filter: usize) {
        assert!(!self.locked.get());
        self.push(Event::Release(filter));
    }
}

fn no_log(function: &'static CStr, parameter: &'static CStr) {
    panic!("logged {function:?}: {parameter:?}");
}

/// Mirrors null_arguments.
#[test]
fn null_arguments_log_once_and_do_nothing() {
    let model = Model::new(3, 0b111);
    let mut logs = Vec::new();

    remove(None::<&Model>, Some(0), |f, p| logs.push((f, p)));
    remove(Some(&model), None, |f, p| logs.push((f, p)));
    // The source is checked first.
    remove(None::<&Model>, None, |f, p| logs.push((f, p)));

    assert_eq!(
        logs,
        [
            (REMOVE_FUNCTION, SOURCE_PARAMETER),
            (REMOVE_FUNCTION, FILTER_PARAMETER),
            (REMOVE_FUNCTION, SOURCE_PARAMETER),
        ]
    );
    // Nothing of the source was read, locked, written, signalled or released.
    assert!(model.events().is_empty());
    assert_eq!(REMOVE_FUNCTION, c"obs_source_filter_remove");
    assert_eq!(SOURCE_PARAMETER, c"source");
    assert_eq!(FILTER_PARAMETER, c"filter");
}

/// Mirrors unlisted_filter_changes_nothing: the lookup reads every entry
/// under the lock, finds nothing, and unlocks.
#[test]
fn unlisted_filter_changes_nothing() {
    for count in 0..=FILTERS {
        let model = Model::new(count, 0b1010101);
        let before = model.chain();
        remove(Some(&model), Some(OUTSIDER), no_log);
        let mut expected = vec![Event::Lock];
        for index in 0..count {
            expected.extend([Event::Count, Event::At(index)]);
        }
        expected.extend([Event::Count, Event::Unlock]);
        assert_eq!(model.events(), expected, "{count}");
        assert_eq!(model.chain(), before);

        let model = Model::new(count, 0);
        assert!(!remove_refless(&model, OUTSIDER));
        assert_eq!(model.events(), expected, "{count}");
    }
}

/// The C's calls for removing the filter at `idx` of a fresh chain of
/// `count`, through obs_source_filter_remove() when `release` is set.
fn expected_events(count: usize, idx: usize, callback: bool, release: bool) -> Vec<Event> {
    use Event::*;
    let mut events = vec![Lock];
    for index in 0..=idx {
        events.extend([Count, At(index)]);
    }
    let filter = idx;
    if idx > 0 {
        let target = if idx + 1 == count { SOURCE } else { idx + 1 };
        events.extend([At(idx - 1), Target(filter), SetTarget(idx - 1, target)]);
    }
    events.extend([
        Erase(idx),
        Unlock,
        Signal(filter),
        Log(filter),
        HasCallback(filter),
    ]);
    if callback {
        events.push(Callback(filter));
    }
    events.push(Clear(filter));
    if release {
        events.push(Release(filter));
    }
    events
}

/// Mirrors remove_middle_retargets_the_previous_entry,
/// remove_first_entry_retargets_nothing and
/// remove_last_entry_hands_over_the_source, on every chain of up to 7
/// filters, every entry, with and without the callback, through both entry
/// points.
#[test]
fn every_entry_is_removed_in_the_c_order() {
    for count in 1..=FILTERS {
        for idx in 0..count {
            for callbacks in [0, u32::MAX, 1 << idx, !(1 << idx)] {
                let callback = callbacks & (1 << idx) != 0;
                for release in [true, false] {
                    let model = Model::new(count, callbacks);
                    if release {
                        remove(Some(&model), Some(idx), no_log);
                    } else {
                        assert!(remove_refless(&model, idx));
                    }
                    assert_eq!(
                        model.events(),
                        expected_events(count, idx, callback, release),
                        "{count} {idx} {callbacks:#b} {release}"
                    );
                    // The rest of the chain still ends at the source.
                    let rest: Vec<usize> = (0..count).filter(|f| *f != idx).collect();
                    let chain: Vec<(usize, usize)> = rest
                        .iter()
                        .enumerate()
                        .map(|(i, f)| (*f, rest.get(i + 1).copied().unwrap_or(SOURCE)))
                        .collect();
                    assert_eq!(model.chain(), chain);
                    assert_eq!(model.targets.borrow()[idx], NULL);
                    assert!(!model.parents.borrow()[idx]);
                }
            }
        }
    }
}

/// Mirrors remove_middle_retargets_the_previous_entry by hand: c, b, a;
/// removing b leaves c targeting a.
#[test]
fn middle_entry_hands_its_target_to_the_one_before() {
    let model = Model::new(3, 0b111);
    remove(Some(&model), Some(1), no_log);
    assert_eq!(model.chain(), [(0, 2), (2, SOURCE)]);
    assert_eq!(model.events().last(), Some(&Event::Release(1)));
    // A second removal finds nothing and releases nothing.
    let events = model.events().len();
    remove(Some(&model), Some(1), no_log);
    assert!(
        !model.events()[events..]
            .iter()
            .any(|e| matches!(e, Event::Release(_) | Event::Signal(_)))
    );
}

/// Mirrors destroy_removes_every_filter: removing the first entry until the
/// list is empty signals each filter in list order, and the last one hands
/// nothing over.
#[test]
fn draining_from_the_first_entry_removes_each_once() {
    let model = Model::new(4, 0b0101);
    while !model.list.borrow().is_empty() {
        let first = model.list.borrow()[0];
        remove(Some(&model), Some(first), no_log);
    }
    let signals: Vec<usize> = model
        .events()
        .iter()
        .filter_map(|e| match e {
            Event::Signal(f) => Some(*f),
            _ => None,
        })
        .collect();
    assert_eq!(signals, [0, 1, 2, 3]);
    let callbacks: Vec<usize> = model
        .events()
        .iter()
        .filter_map(|e| match e {
            Event::Callback(f) => Some(*f),
            _ => None,
        })
        .collect();
    assert_eq!(callbacks, [0, 2]);
    assert!(
        !model
            .events()
            .iter()
            .any(|e| matches!(e, Event::SetTarget(..)))
    );
}
