//! Tier 1 mirrors of test_source_enum_filters.c without FFI or unsafe code.
use core::ffi::CStr;
use obs_source::enum_filters::{
    CALLBACK_PARAMETER, ENUM_FILTERS_FUNCTION, FilterList, SOURCE_PARAMETER, enum_filters,
};
use std::cell::{Cell, RefCell};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Lock,
    Count,
    At(usize),
    Unlock,
    Call(usize),
}

/// A model source: its list holds filter ids, and every read is recorded in
/// order.
struct Model {
    list: RefCell<Vec<usize>>,
    /// `filters.num`, apart from the array as in the C.
    num: Cell<usize>,
    locked: Cell<bool>,
    events: RefCell<Vec<Event>>,
}

impl Model {
    fn new(list: &[usize]) -> Self {
        Self {
            list: RefCell::new(list.to_vec()),
            num: Cell::new(list.len()),
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
}

impl FilterList for Model {
    type Filter = usize;
    fn lock(&self) {
        assert!(!self.locked.replace(true), "locked twice");
        self.push(Event::Lock);
    }
    fn count(&self) -> usize {
        assert!(self.locked.get());
        self.push(Event::Count);
        self.num.get()
    }
    fn filter(&self, index: usize) -> usize {
        assert!(self.locked.get());
        self.push(Event::At(index));
        self.list.borrow()[index]
    }
    fn unlock(&self) {
        assert!(self.locked.replace(false), "unlocked while not locked");
        self.push(Event::Unlock);
    }
}

fn no_log(function: &'static CStr, parameter: &'static CStr) {
    panic!("logged {function:?}: {parameter:?}");
}

#[test]
fn null_arguments_log_once_and_return() {
    let model = Model::new(&[0, 1, 2]);
    let mut logs = Vec::new();

    enum_filters::<Model>(None, Some(|_| panic!("called")), |f, p| logs.push((f, p)));
    // The source is checked first.
    enum_filters::<Model>(None, None::<fn(usize)>, |f, p| logs.push((f, p)));
    enum_filters(Some(&model), None::<fn(usize)>, |f, p| logs.push((f, p)));

    assert_eq!(
        logs,
        [
            (ENUM_FILTERS_FUNCTION, SOURCE_PARAMETER),
            (ENUM_FILTERS_FUNCTION, SOURCE_PARAMETER),
            (ENUM_FILTERS_FUNCTION, CALLBACK_PARAMETER),
        ]
    );
    // Nothing of the source was read.
    assert!(model.events().is_empty());
    assert_eq!(ENUM_FILTERS_FUNCTION, c"obs_source_enum_filters");
    assert_eq!(SOURCE_PARAMETER, c"source");
    assert_eq!(CALLBACK_PARAMETER, c"callback_obj.obj");
}

#[test]
fn enum_filters_walks_last_to_first_under_the_lock() {
    for count in 0..=6 {
        let list: Vec<usize> = (0..count).collect();
        let model = Model::new(&list);
        let mut seen = Vec::new();
        enum_filters(
            Some(&model),
            Some(|filter| {
                assert!(model.locked.get());
                model.push(Event::Call(filter));
                seen.push(filter);
            }),
            no_log,
        );
        let expected: Vec<usize> = list.iter().rev().copied().collect();
        assert_eq!(seen, expected);
        let mut events = vec![Event::Lock, Event::Count];
        for index in (0..count).rev() {
            events.extend([Event::At(index), Event::Call(index)]);
        }
        events.push(Event::Unlock);
        assert_eq!(model.events(), events);
        assert!(!model.locked.get());
    }
}

#[test]
fn enum_filters_reads_the_count_once_and_each_entry_when_visited() {
    // The callback swaps the list: the remaining calls read the new one at
    // the same indexes.
    let model = Model::new(&[0, 1, 2, 3]);
    let mut seen = Vec::new();
    enum_filters(
        Some(&model),
        Some(|filter| {
            if seen.is_empty() {
                *model.list.borrow_mut() = vec![5, 4, 3, 2];
            }
            seen.push(filter);
        }),
        no_log,
    );
    assert_eq!(seen, [3, 3, 4, 5]);

    // The callback shrinks the list's count: every call still happens, at
    // the indexes the first count gave.
    let model = Model::new(&[0, 1, 2, 3]);
    let mut seen = Vec::new();
    enum_filters(
        Some(&model),
        Some(|filter| {
            if seen.is_empty() {
                // The entries past the new count stay readable, as in the C
                // array.
                model.num.set(1);
            }
            seen.push(filter);
        }),
        no_log,
    );
    assert_eq!(seen, [3, 2, 1, 0]);
    assert_eq!(
        model
            .events()
            .iter()
            .filter(|e| **e == Event::Count)
            .count(),
        1
    );

    // The callback replaces the next entry: the replacement is visited.
    let model = Model::new(&[0, 1, 2]);
    let mut seen = Vec::new();
    enum_filters(
        Some(&model),
        Some(|filter| {
            if seen.is_empty() {
                model.list.borrow_mut()[1] = 5;
            }
            seen.push(filter);
        }),
        no_log,
    );
    assert_eq!(seen, [2, 5, 0]);
}
