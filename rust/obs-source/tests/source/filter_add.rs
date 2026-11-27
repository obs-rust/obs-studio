//! Tier 1 mirrors of test_source_filter_add.c without FFI or unsafe code.
use core::ffi::CStr;
use obs_source::filter_add::{ADD_FUNCTION, FILTER_PARAMETER, FilterList, SOURCE_PARAMETER, add};
use obs_source::source_filter_chain::{ASYNC, AUDIO, VIDEO};
use std::cell::{Cell, RefCell};

/// The longest chain a case starts with.
const CHAIN: usize = 6;
/// The filter a case adds (on no list unless a case puts it there).
const NEW: usize = CHAIN;
/// Every filter id, the chain's and the new one.
const FILTERS: usize = CHAIN + 1;
/// The handle of the source itself.
const SOURCE: usize = 100;
/// The handle of another source, a parent elsewhere.
const OTHER: usize = 101;
/// No parent, no target.
const NONE: usize = 102;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Lock,
    Count,
    At(usize),
    HasParent(usize),
    WarnPresent,
    OwnsInfoId,
    OutputFlags,
    FilterFlags(usize),
    GetRef(usize),
    SetParent(usize),
    SetTarget(usize, usize),
    InsertFront(usize),
    Unlock,
    Signal(usize),
    Log(usize),
    HasCallback(usize),
    Callback(usize),
}

/// A model source: its list holds filter ids (filter `i` targets the next
/// one, the last the source), filter `i` has a `filter_add` callback when
/// bit `i` of the mask is set, and every call is recorded.
struct Model {
    list: RefCell<Vec<usize>>,
    targets: RefCell<Vec<usize>>,
    parents: RefCell<Vec<usize>>,
    flags: Vec<u32>,
    source_flags: u32,
    owns_info_id: bool,
    dying: Option<usize>,
    callbacks: u32,
    locked: Cell<bool>,
    events: RefCell<Vec<Event>>,
}

impl Model {
    fn new(count: usize, callbacks: u32) -> Self {
        let mut targets: Vec<usize> = (1..=FILTERS).collect();
        targets[NEW] = NONE;
        if count > 0 {
            targets[count - 1] = SOURCE;
        }
        Self {
            list: RefCell::new((0..count).collect()),
            targets: RefCell::new(targets),
            parents: RefCell::new(
                (0..FILTERS)
                    .map(|i| if i < count { SOURCE } else { NONE })
                    .collect(),
            ),
            flags: vec![VIDEO; FILTERS],
            source_flags: VIDEO,
            owns_info_id: false,
            dying: None,
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
    fn this(&self) -> usize {
        SOURCE
    }
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
    fn has_parent(&self, filter: usize) -> bool {
        assert!(self.locked.get());
        self.push(Event::HasParent(filter));
        self.parents.borrow()[filter] != NONE
    }
    fn warn_present(&self) {
        assert!(self.locked.get(), "warned outside the lock");
        self.push(Event::WarnPresent);
    }
    fn owns_info_id(&self) -> bool {
        assert!(self.locked.get());
        self.push(Event::OwnsInfoId);
        self.owns_info_id
    }
    fn output_flags(&self) -> u32 {
        assert!(self.locked.get());
        self.push(Event::OutputFlags);
        self.source_flags
    }
    fn filter_output_flags(&self, filter: usize) -> u32 {
        assert!(self.locked.get());
        self.push(Event::FilterFlags(filter));
        self.flags[filter]
    }
    fn get_ref(&self, filter: usize) -> Option<usize> {
        assert!(self.locked.get());
        self.push(Event::GetRef(filter));
        (self.dying != Some(filter)).then_some(filter)
    }
    fn set_parent(&self, filter: usize) {
        assert!(self.locked.get());
        self.push(Event::SetParent(filter));
        self.parents.borrow_mut()[filter] = SOURCE;
    }
    fn set_target(&self, filter: usize, target: usize) {
        assert!(self.locked.get());
        self.push(Event::SetTarget(filter, target));
        self.targets.borrow_mut()[filter] = target;
    }
    fn insert_front(&self, filter: usize) {
        assert!(self.locked.get());
        self.push(Event::InsertFront(filter));
        self.list.borrow_mut().insert(0, filter);
    }
    fn unlock(&self) {
        assert!(self.locked.replace(false), "unlocked while not locked");
        self.push(Event::Unlock);
    }
    fn signal(&self, filter: usize) {
        assert!(!self.locked.get(), "signalled under the lock");
        // The list already holds the filter at the front, parented.
        assert_eq!(self.list.borrow().first(), Some(&filter));
        assert_eq!(self.parents.borrow()[filter], SOURCE);
        self.push(Event::Signal(filter));
    }
    fn log_added(&self, filter: usize) {
        assert!(!self.locked.get());
        self.push(Event::Log(filter));
    }
    fn has_add_callback(&self, filter: usize) -> bool {
        assert!(!self.locked.get());
        self.push(Event::HasCallback(filter));
        self.callbacks & (1 << filter) != 0
    }
    fn call_add_callback(&self, filter: usize) {
        assert!(!self.locked.get());
        assert_eq!(self.parents.borrow()[filter], SOURCE);
        self.push(Event::Callback(filter));
    }
}

fn no_log(function: &'static CStr, parameter: &'static CStr) {
    panic!("logged {function:?}: {parameter:?}");
}

/// The lookup of `filter` on a list of `count` that does not hold it.
fn missed_lookup(count: usize, filter: usize) -> Vec<Event> {
    let mut events = vec![Event::Lock];
    for index in 0..count {
        events.extend([Event::Count, Event::At(index)]);
    }
    events.extend([Event::Count, Event::HasParent(filter)]);
    events
}

/// The C's calls for adding the new filter to a fresh chain of `count`.
fn expected_add(count: usize, owns_info_id: bool, callback: bool) -> Vec<Event> {
    use Event::*;
    let mut events = missed_lookup(count, NEW);
    events.push(OwnsInfoId);
    if !owns_info_id {
        events.extend([OutputFlags, FilterFlags(NEW)]);
    }
    events.extend([GetRef(NEW), SetParent(NEW), Count]);
    let target = if count == 0 {
        SOURCE
    } else {
        events.push(At(0));
        0
    };
    events.extend([
        SetTarget(NEW, target),
        InsertFront(NEW),
        Unlock,
        Signal(NEW),
        Log(NEW),
        HasCallback(NEW),
    ]);
    if callback {
        events.push(Callback(NEW));
    }
    events
}

/// Mirrors null_arguments.
#[test]
fn null_arguments_log_once_and_do_nothing() {
    let model = Model::new(3, u32::MAX);
    let mut logs = Vec::new();

    add(None::<&Model>, Some(NEW), |f, p| logs.push((f, p)));
    add(Some(&model), None, |f, p| logs.push((f, p)));
    // The source is checked first.
    add(None::<&Model>, None, |f, p| logs.push((f, p)));

    assert_eq!(
        logs,
        [
            (ADD_FUNCTION, SOURCE_PARAMETER),
            (ADD_FUNCTION, FILTER_PARAMETER),
            (ADD_FUNCTION, SOURCE_PARAMETER),
        ]
    );
    // Nothing of the source was read, locked, written or signalled.
    assert!(model.events().is_empty());
    assert_eq!(ADD_FUNCTION, c"obs_source_filter_add");
    assert_eq!(SOURCE_PARAMETER, c"source");
    assert_eq!(FILTER_PARAMETER, c"filter");
}

/// Mirrors add_builds_the_chain_from_the_front on every chain of up to 6
/// filters, with and without the callback, on a source that owns its info
/// id or not.
#[test]
fn new_filter_goes_to_the_front_in_the_c_order() {
    for count in 0..=CHAIN {
        for callbacks in [0, u32::MAX] {
            for owns_info_id in [false, true] {
                let mut model = Model::new(count, callbacks);
                model.owns_info_id = owns_info_id;
                add(Some(&model), Some(NEW), no_log);
                assert_eq!(
                    model.events(),
                    expected_add(count, owns_info_id, callbacks != 0),
                    "{count} {callbacks:#b} {owns_info_id}"
                );
                let mut chain = vec![(NEW, if count == 0 { SOURCE } else { 0 })];
                chain.extend((0..count).map(|f| (f, if f + 1 == count { SOURCE } else { f + 1 })));
                assert_eq!(model.chain(), chain);
            }
        }
    }
}

/// Mirrors listed_filter_warns: every listed entry warns under the lock and
/// changes nothing. So does a filter parented elsewhere, by the add rule of
/// `source_filter_chain` (the original C took it; test_source_filter_add
/// leaves that case out because the two libraries differ there).
#[test]
fn listed_or_parented_filter_warns_under_the_lock() {
    for count in 1..=CHAIN {
        for idx in 0..count {
            let model = Model::new(count, u32::MAX);
            let before = model.chain();
            add(Some(&model), Some(idx), no_log);
            let mut expected = vec![Event::Lock];
            for index in 0..=idx {
                expected.extend([Event::Count, Event::At(index)]);
            }
            expected.extend([Event::HasParent(idx), Event::WarnPresent, Event::Unlock]);
            assert_eq!(model.events(), expected, "{count} {idx}");
            assert_eq!(model.chain(), before);
        }
    }
    for count in 0..=CHAIN {
        let model = Model::new(count, u32::MAX);
        model.parents.borrow_mut()[NEW] = OTHER;
        let before = model.chain();
        add(Some(&model), Some(NEW), no_log);
        let mut expected = missed_lookup(count, NEW);
        expected.extend([Event::WarnPresent, Event::Unlock]);
        assert_eq!(model.events(), expected, "{count}");
        assert_eq!(model.chain(), before);
        assert_eq!(model.parents.borrow()[NEW], OTHER);
    }
}

/// Mirrors incompatible_filter_is_refused_silently, over the flag
/// combinations of the compatibility rule: refused filters unlock and stop
/// without a line; a source that owns its info id takes them.
#[test]
fn incompatible_filter_is_refused_unless_the_source_owns_its_id() {
    let flags = [
        0,
        VIDEO,
        AUDIO,
        ASYNC,
        VIDEO | AUDIO,
        VIDEO | ASYNC,
        AUDIO | ASYNC,
        VIDEO | AUDIO | ASYNC,
    ];
    for source_flags in flags {
        for filter_flags in flags {
            let fits = obs_source::source_filter_chain::compatible(source_flags, filter_flags);
            for owns_info_id in [false, true] {
                let mut model = Model::new(2, u32::MAX);
                model.source_flags = source_flags;
                model.flags[NEW] = filter_flags;
                model.owns_info_id = owns_info_id;
                add(Some(&model), Some(NEW), no_log);
                if fits || owns_info_id {
                    assert_eq!(model.events(), expected_add(2, owns_info_id, true));
                    assert_eq!(model.list(), [NEW, 0, 1]);
                } else {
                    let mut expected = missed_lookup(2, NEW);
                    expected.extend([
                        Event::OwnsInfoId,
                        Event::OutputFlags,
                        Event::FilterFlags(NEW),
                        Event::Unlock,
                    ]);
                    assert_eq!(model.events(), expected, "{source_flags} {filter_flags}");
                    assert_eq!(model.list(), [0, 1]);
                    assert_eq!(model.parents.borrow()[NEW], NONE);
                }
            }
        }
    }
}

/// Mirrors dying_filter_is_not_added: the reference fails, the 'filter'
/// line is logged, the lock released (the C returned with it held, #2981)
/// and nothing changes.
#[test]
fn dying_filter_logs_and_unlocks() {
    for count in 0..=CHAIN {
        let mut model = Model::new(count, u32::MAX);
        model.dying = Some(NEW);
        let before = model.chain();
        let mut logs = Vec::new();
        add(Some(&model), Some(NEW), |f, p| logs.push((f, p)));
        assert_eq!(logs, [(ADD_FUNCTION, FILTER_PARAMETER)]);
        let mut expected = missed_lookup(count, NEW);
        expected.extend([
            Event::OwnsInfoId,
            Event::OutputFlags,
            Event::FilterFlags(NEW),
            Event::GetRef(NEW),
            Event::Unlock,
        ]);
        assert_eq!(model.events(), expected, "{count}");
        assert!(!model.locked.get());
        assert_eq!(model.chain(), before);
        assert_eq!(model.parents.borrow()[NEW], NONE);
    }
}

/// Adding several filters one after another builds the chain from the
/// front, each targeting the one added before it.
#[test]
fn successive_adds_chain_each_to_the_previous() {
    let model = Model::new(0, 0b101);
    for filter in 0..3 {
        add(Some(&model), Some(filter), no_log);
    }
    assert_eq!(model.chain(), [(2, 1), (1, 0), (0, SOURCE)]);
    let callbacks: Vec<usize> = model
        .events()
        .iter()
        .filter_map(|e| match e {
            Event::Callback(f) => Some(*f),
            _ => None,
        })
        .collect();
    assert_eq!(callbacks, [0, 2]);
}
