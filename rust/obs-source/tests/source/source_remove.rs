//! Tier 1 mirrors of test_source_remove.c without FFI or unsafe code.
use core::ffi::CStr;
use obs_source::source_remove::{REMOVE_FUNCTION, SOURCE_PARAMETER, Source, remove};
use std::cell::{Cell, RefCell};

/// The source passed in.
const SOURCE: usize = 0;
/// Another source a reference could name (never in libobs, where
/// obs_source_get_ref() returns its argument; here it tells which pointer
/// each step uses).
const TWIN: usize = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Removed,
    GetRef,
    SetRemoved(usize),
    /// The source signalled, and whether it was already marked removed.
    Signal(usize, bool),
    HasCanvas,
    Private,
    RemoveFromCanvas(usize),
    Release(usize),
}

/// A model source: its removed flag (and the twin's), what get_ref returns,
/// whether it has a canvas and is private; every call is recorded.
struct Model {
    removed: [Cell<bool>; 2],
    reference: Option<usize>,
    canvas: bool,
    private: bool,
    events: RefCell<Vec<Event>>,
}

impl Model {
    fn new(removed: bool, reference: Option<usize>, canvas: bool, private: bool) -> Self {
        Self {
            removed: [Cell::new(removed), Cell::new(false)],
            reference,
            canvas,
            private,
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

impl Source for Model {
    type Ref = usize;
    fn removed(&self) -> bool {
        self.push(Event::Removed);
        self.removed[SOURCE].get()
    }
    fn get_ref(&self) -> Option<usize> {
        self.push(Event::GetRef);
        self.reference
    }
    fn set_removed(&self, s: usize) {
        self.push(Event::SetRemoved(s));
        self.removed[s].set(true);
    }
    fn signal(&self, s: usize) {
        self.push(Event::Signal(s, self.removed[s].get()));
    }
    fn has_canvas(&self) -> bool {
        self.push(Event::HasCanvas);
        self.canvas
    }
    fn is_private(&self) -> bool {
        self.push(Event::Private);
        self.private
    }
    fn remove_from_canvas(&self, s: usize) {
        self.push(Event::RemoveFromCanvas(s));
    }
    fn release(&self, s: usize) {
        self.push(Event::Release(s));
    }
}

fn no_log(function: &'static CStr, parameter: &'static CStr) {
    panic!("logged {function:?}: {parameter:?}");
}

/// Mirrors null_source_logs_once.
#[test]
fn null_source_logs_once() {
    let mut logs = Vec::new();
    remove(None::<&Model>, |f, p| logs.push((f, p)));
    assert_eq!(logs, [(REMOVE_FUNCTION, SOURCE_PARAMETER)]);
    assert_eq!(REMOVE_FUNCTION, c"obs_source_remove");
    assert_eq!(SOURCE_PARAMETER, c"source");
}

/// The C's calls for a source not yet removed whose reference is `s`.
fn expected_live(s: usize, canvas: bool, private: bool) -> Vec<Event> {
    use Event::*;
    let mut events = vec![Removed, GetRef, SetRemoved(s), Signal(s, true), HasCanvas];
    if canvas {
        events.push(Private);
        if !private {
            events.push(RemoveFromCanvas(s));
        }
    }
    events.push(Release(s));
    events
}

/// Mirrors public_source_without_canvas, public_scene_leaves_its_canvas and
/// private_scene_keeps_its_canvas: every canvas and private pattern. The
/// private flag is read only when there is a canvas.
#[test]
fn live_source_is_removed_in_the_c_order() {
    for canvas in [false, true] {
        for private in [false, true] {
            let model = Model::new(false, Some(SOURCE), canvas, private);
            remove(Some(&model), no_log);
            assert_eq!(
                model.events(),
                expected_live(SOURCE, canvas, private),
                "{canvas} {private}"
            );
            assert!(model.removed[SOURCE].get());
        }
    }
}

/// The checks read the source passed in; the steps act on the reference.
#[test]
fn checks_read_the_argument_and_steps_use_the_reference() {
    for canvas in [false, true] {
        for private in [false, true] {
            let model = Model::new(false, Some(TWIN), canvas, private);
            remove(Some(&model), no_log);
            assert_eq!(
                model.events(),
                expected_live(TWIN, canvas, private),
                "{canvas} {private}"
            );
            assert!(model.removed[TWIN].get());
            assert!(!model.removed[SOURCE].get());
        }
    }
}

/// Mirrors removed_source_changes_nothing: only the flag is read.
#[test]
fn removed_source_changes_nothing() {
    for canvas in [false, true] {
        for private in [false, true] {
            let model = Model::new(true, Some(SOURCE), canvas, private);
            remove(Some(&model), no_log);
            assert_eq!(model.events(), [Event::Removed]);
        }
    }
}

/// A second call on the same source is a no-op.
#[test]
fn second_call_is_a_no_op() {
    let model = Model::new(false, Some(SOURCE), true, false);
    remove(Some(&model), no_log);
    let first = model.events().len();
    remove(Some(&model), no_log);
    assert_eq!(model.events()[first..], [Event::Removed]);
}

/// Mirrors released_source_gets_no_reference: no reference, nothing else.
#[test]
fn no_reference_changes_nothing() {
    for canvas in [false, true] {
        for private in [false, true] {
            let model = Model::new(false, None, canvas, private);
            remove(Some(&model), no_log);
            assert_eq!(model.events(), [Event::Removed, Event::GetRef]);
            assert!(!model.removed[SOURCE].get());
        }
    }
}
