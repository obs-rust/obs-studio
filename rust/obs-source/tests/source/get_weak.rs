//! Tier 1: mirror test_source_get_weak.c over safe source and control tokens.
use obs_source::get_weak::{Host, get_weak_source};
use std::cell::RefCell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SourceId(usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ControlId(usize);

/// A source with an optional control, and that control's counters.
#[derive(Clone, Debug, PartialEq, Eq)]
struct State {
    control: Option<ControlId>,
    refs: i64,
    weak_refs: i64,
    target: Option<SourceId>,
    logs: u32,
    calls: Vec<&'static str>,
}

struct Probe(RefCell<State>);

impl Host for Probe {
    type Source = SourceId;
    type Weak = ControlId;
    fn control(&self, _source: SourceId) -> Option<ControlId> {
        let mut state = self.0.borrow_mut();
        state.calls.push("control");
        state.control
    }
    fn addref(&self, weak: Option<ControlId>) {
        let mut state = self.0.borrow_mut();
        state.calls.push("addref");
        // obs_weak_source_addref is a no-op for NULL.
        if weak.is_some() {
            state.weak_refs += 1;
        }
    }
}

fn probe(control: Option<ControlId>) -> Probe {
    Probe(RefCell::new(State {
        control,
        refs: 7,
        weak_refs: 3,
        target: Some(SourceId(0x5228)),
        logs: 0,
        calls: Vec::new(),
    }))
}

/// A NULL source returns NULL, logs nothing and reads nothing.
#[test]
fn test_source_get_weak_null_source() {
    let host = probe(Some(ControlId(1)));
    let before = host.0.borrow().clone();
    assert_eq!(get_weak_source(&host, None), None);
    assert_eq!(*host.0.borrow(), before);
}

/// The result is the source's own control, one weak reference per call, with
/// the strong count, the control's source and the source itself untouched.
#[test]
fn test_source_get_weak_returns_control_with_one_weak_ref() {
    let control = ControlId(0xc0de);
    let host = probe(Some(control));
    for i in 1..=3 {
        assert_eq!(
            get_weak_source(&host, Some(SourceId(0x5228))),
            Some(control)
        );
        let state = host.0.borrow();
        assert_eq!(state.weak_refs, 3 + i);
        assert_eq!(state.refs, 7);
        assert_eq!(state.target, Some(SourceId(0x5228)));
        assert_eq!(state.control, Some(control));
    }
    let state = host.0.borrow();
    assert_eq!(state.logs, 0);
    assert_eq!(state.calls, ["control", "addref"].repeat(3));
}

/// A source with no control returns NULL through the no-op addref.
#[test]
fn test_source_get_weak_null_control() {
    let host = probe(None);
    assert_eq!(get_weak_source(&host, Some(SourceId(1))), None);
    let state = host.0.borrow();
    assert_eq!(state.control, None);
    assert_eq!(state.weak_refs, 3);
    assert_eq!(state.logs, 0);
    assert_eq!(state.calls, ["control", "addref"]);
}

/// Two acquisitions on a live source return the same control and add two
/// weak references without touching the strong count; teardown and release
/// remain C-owned, so the handles are only tokens here.
#[test]
fn test_source_get_weak_live_source() {
    let control = ControlId(0x11fe);
    let host = probe(Some(control));
    let first = get_weak_source(&host, Some(SourceId(2)));
    let second = get_weak_source(&host, Some(SourceId(2)));
    assert_eq!(first, Some(control));
    assert_eq!(second, Some(control));
    let state = host.0.borrow();
    assert_eq!(state.refs, 7);
    assert_eq!(state.weak_refs, 5);
}
