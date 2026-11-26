//! Tier 1 mirrors of test_source_copy_filters.c without FFI or unsafe
//! code, plus the order of every step and the lock's scope.
use core::ffi::CStr;
use obs_source::copy_filters::{
    COPY_FILTERS_FUNCTION, COPY_SINGLE_FILTER_FUNCTION, DST_PARAMETER, FILTER_PARAMETER,
    FilterCopy, SRC_PARAMETER, copy_filters, copy_single_filter, duplicate_filters,
};
use std::cell::{Cell, RefCell};

/// Source ids: the destination, the source, then filters from `FIRST`.
const DST: u32 = 1;
const SRC: u32 = 2;
const FIRST: u32 = 10;
/// A duplicate's id is its filter's plus this; `NULL_COPY` is a NULL one.
const COPY: u32 = 100;
const NULL_COPY: u32 = 0;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    Private(u32),
    Lock(u32),
    Count(u32),
    At(u32, usize),
    GetRef(u32),
    Unlock(u32),
    Compatible(u32, u32),
    NewName(u32, u32),
    Enabled(u32),
    Duplicate(u32, String, bool),
    SetEnabled(u32, bool),
    FreeName(String),
    Add(u32, u32),
    Release(u32),
}

/// A model of libobs: `src`'s filters, which of them are going away (their
/// reference fails), which are disabled, incompatible or refuse to be
/// duplicated, and every call in order.
struct Model {
    filters: Vec<u32>,
    dying: Vec<u32>,
    disabled: Vec<u32>,
    incompatible: Vec<u32>,
    no_duplicate: Vec<u32>,
    private: bool,
    locked: Cell<bool>,
    names: Cell<u32>,
    events: RefCell<Vec<Event>>,
}

impl Model {
    fn new(count: u32) -> Self {
        Self {
            filters: (FIRST..FIRST + count).collect(),
            dying: Vec::new(),
            disabled: Vec::new(),
            incompatible: Vec::new(),
            no_duplicate: Vec::new(),
            private: false,
            locked: Cell::new(false),
            names: Cell::new(0),
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

impl FilterCopy for Model {
    type Source = u32;
    type Name = String;
    fn private(&self, dst: u32) -> bool {
        self.push(Event::Private(dst));
        self.private
    }
    fn lock(&self, src: u32) {
        assert!(!self.locked.replace(true), "locked twice");
        self.push(Event::Lock(src));
    }
    fn count(&self, src: u32) -> usize {
        assert!(self.locked.get(), "count read unlocked");
        self.push(Event::Count(src));
        self.filters.len()
    }
    fn filter(&self, src: u32, index: usize) -> u32 {
        assert!(self.locked.get(), "entry read unlocked");
        self.push(Event::At(src, index));
        self.filters[index]
    }
    fn get_ref(&self, filter: u32) -> Option<u32> {
        assert!(self.locked.get(), "reference taken unlocked");
        self.push(Event::GetRef(filter));
        (!self.dying.contains(&filter)).then_some(filter)
    }
    fn unlock(&self, src: u32) {
        assert!(self.locked.replace(false), "unlocked while not locked");
        self.push(Event::Unlock(src));
    }
    fn compatible(&self, dst: u32, filter: u32) -> bool {
        self.push(Event::Compatible(dst, filter));
        !self.incompatible.contains(&filter)
    }
    fn new_name(&self, dst: u32, filter: u32) -> String {
        assert!(!self.locked.get(), "named under the lock");
        self.push(Event::NewName(dst, filter));
        let n = self.names.get();
        self.names.set(n + 1);
        format!("name{n}")
    }
    fn enabled(&self, filter: u32) -> bool {
        self.push(Event::Enabled(filter));
        !self.disabled.contains(&filter)
    }
    fn duplicate(&self, filter: u32, name: &String, private: bool) -> u32 {
        assert!(!self.locked.get(), "duplicated under the lock");
        self.push(Event::Duplicate(filter, name.clone(), private));
        if self.no_duplicate.contains(&filter) {
            NULL_COPY
        } else {
            filter + COPY
        }
    }
    fn set_enabled(&self, copy: u32, enabled: bool) {
        self.push(Event::SetEnabled(copy, enabled));
    }
    fn free_name(&self, name: String) {
        self.push(Event::FreeName(name));
    }
    fn add(&self, dst: u32, copy: u32) {
        assert!(!self.locked.get(), "added under the lock");
        self.push(Event::Add(dst, copy));
    }
    fn release(&self, source: u32) {
        assert!(!self.locked.get(), "released under the lock");
        self.push(Event::Release(source));
    }
}

fn no_log(function: &'static CStr, parameter: &'static CStr) {
    panic!("logged {function:?}: {parameter:?}");
}

/// The calls of one filter's copy in `duplicate_filters()`: named, read,
/// duplicated, enabled, freed, added, both released.
fn copy_steps(name: u32, filter: u32, enabled: bool, private: bool, copy: u32) -> Vec<Event> {
    let name = format!("name{name}");
    vec![
        Event::NewName(DST, filter),
        Event::Enabled(filter),
        Event::Duplicate(filter, name.clone(), private),
        Event::SetEnabled(copy, enabled),
        Event::FreeName(name),
        Event::Add(DST, copy),
        Event::Release(copy),
        Event::Release(filter),
    ]
}

/// The snapshot: under the lock, the count before each entry and a
/// reference to each.
fn snapshot(count: u32) -> Vec<Event> {
    let mut events = vec![Event::Lock(SRC), Event::Count(SRC)];
    for i in 0..count {
        let index = usize::try_from(i).unwrap();
        events.extend([
            Event::Count(SRC),
            Event::At(SRC, index),
            Event::GetRef(FIRST + i),
        ]);
    }
    events.extend([Event::Count(SRC), Event::Unlock(SRC)]);
    events
}

/// Mirrors null_arguments: the destination is checked first, and nothing
/// of either source is touched.
#[test]
fn null_arguments() {
    let model = Model::new(2);
    let mut logs = Vec::new();

    copy_filters(&model, None, Some(SRC), |f, p| logs.push((f, p)));
    copy_filters(&model, Some(DST), None, |f, p| logs.push((f, p)));
    copy_filters(&model, None, None, |f, p| logs.push((f, p)));
    copy_single_filter(&model, None, Some(FIRST), |f, p| logs.push((f, p)));
    copy_single_filter(&model, Some(DST), None, |f, p| logs.push((f, p)));
    copy_single_filter(&model, None, None, |f, p| logs.push((f, p)));

    assert_eq!(
        logs,
        [
            (COPY_FILTERS_FUNCTION, DST_PARAMETER),
            (COPY_FILTERS_FUNCTION, SRC_PARAMETER),
            (COPY_FILTERS_FUNCTION, DST_PARAMETER),
            (COPY_SINGLE_FILTER_FUNCTION, DST_PARAMETER),
            (COPY_SINGLE_FILTER_FUNCTION, FILTER_PARAMETER),
            (COPY_SINGLE_FILTER_FUNCTION, DST_PARAMETER),
        ]
    );
    assert!(model.events().is_empty());
    assert_eq!(COPY_FILTERS_FUNCTION, c"obs_source_copy_filters");
    assert_eq!(
        COPY_SINGLE_FILTER_FUNCTION,
        c"obs_source_copy_single_filter"
    );
    assert_eq!(DST_PARAMETER, c"dst");
    assert_eq!(SRC_PARAMETER, c"src");
    assert_eq!(FILTER_PARAMETER, c"filter");
}

/// Mirrors copy_filters_order_names_and_state and
/// copy_filters_private_destination: the snapshot under the lock, then each
/// filter copied after the unlock from the last, private as the
/// destination is.
#[test]
fn copy_filters_last_to_first_as_private_as_the_destination() {
    for private in [false, true] {
        for count in 0..=6 {
            let mut model = Model::new(count);
            model.private = private;
            model.disabled = vec![FIRST + 1, FIRST + 4];
            copy_filters(&model, Some(DST), Some(SRC), no_log);

            let mut expected = vec![Event::Private(DST)];
            expected.extend(snapshot(count));
            for (n, i) in (0..count).rev().enumerate() {
                let filter = FIRST + i;
                let enabled = !model.disabled.contains(&filter);
                let n = u32::try_from(n).unwrap();
                expected.extend(copy_steps(n, filter, enabled, private, filter + COPY));
            }
            assert_eq!(model.events(), expected, "{private} {count}");
            assert!(!model.locked.get());
        }
    }
}

/// A filter whose reference fails (it is going away) is skipped: not
/// named, copied or released.
#[test]
fn copy_filters_skips_a_dying_filter() {
    let mut model = Model::new(4);
    model.dying = vec![FIRST, FIRST + 2];
    copy_filters(&model, Some(DST), Some(SRC), no_log);

    let mut expected = vec![Event::Private(DST)];
    expected.extend(snapshot(4));
    expected.extend(copy_steps(0, FIRST + 3, true, false, FIRST + 3 + COPY));
    expected.extend(copy_steps(1, FIRST + 1, true, false, FIRST + 1 + COPY));
    assert_eq!(model.events(), expected);
}

/// A duplicate that fails still goes through the C's calls, with NULL.
#[test]
fn copy_filters_passes_a_failed_duplicate_on() {
    let mut model = Model::new(2);
    model.no_duplicate = vec![FIRST + 1];
    copy_filters(&model, Some(DST), Some(SRC), no_log);

    let mut expected = vec![Event::Private(DST)];
    expected.extend(snapshot(2));
    expected.extend(copy_steps(0, FIRST + 1, true, false, NULL_COPY));
    expected.extend(copy_steps(1, FIRST, true, false, FIRST + COPY));
    assert_eq!(model.events(), expected);
}

/// obs_source_duplicate()'s path: the private flag is the caller's, and the
/// destination's own flag is never read.
#[test]
fn duplicate_filters_takes_the_callers_private_flag() {
    for private in [false, true] {
        let model = Model::new(3);
        duplicate_filters(&model, DST, SRC, private);
        let mut expected = snapshot(3);
        for (n, i) in (0..3).rev().enumerate() {
            let n = u32::try_from(n).unwrap();
            expected.extend(copy_steps(n, FIRST + i, true, private, FIRST + i + COPY));
        }
        assert_eq!(model.events(), expected);
    }
}

/// Mirrors copy_single_filter_private_and_named: checked, named, read,
/// duplicated private, enabled, freed, added and the copy released; no
/// lock and no reference on the filter.
#[test]
fn copy_single_filter_copies_once_private() {
    for disabled in [false, true] {
        let mut model = Model::new(1);
        if disabled {
            model.disabled = vec![FIRST];
        }
        copy_single_filter(&model, Some(DST), Some(FIRST), no_log);
        let name = String::from("name0");
        assert_eq!(
            model.events(),
            [
                Event::Compatible(DST, FIRST),
                Event::NewName(DST, FIRST),
                Event::Enabled(FIRST),
                Event::Duplicate(FIRST, name.clone(), true),
                Event::SetEnabled(FIRST + COPY, !disabled),
                Event::FreeName(name),
                Event::Add(DST, FIRST + COPY),
                Event::Release(FIRST + COPY),
            ]
        );
    }
}

/// Mirrors copy_single_filter_incompatible: nothing past the check.
#[test]
fn copy_single_filter_skips_an_incompatible_filter() {
    let mut model = Model::new(1);
    model.incompatible = vec![FIRST];
    copy_single_filter(&model, Some(DST), Some(FIRST), no_log);
    assert_eq!(model.events(), [Event::Compatible(DST, FIRST)]);
}

/// A failed duplicate of the single filter goes on with NULL too.
#[test]
fn copy_single_filter_passes_a_failed_duplicate_on() {
    let mut model = Model::new(1);
    model.no_duplicate = vec![FIRST];
    copy_single_filter(&model, Some(DST), Some(FIRST), no_log);
    let events = model.events();
    assert_eq!(events[4..], [
        Event::SetEnabled(NULL_COPY, true),
        Event::FreeName(String::from("name0")),
        Event::Add(DST, NULL_COPY),
        Event::Release(NULL_COPY),
    ]);
}
