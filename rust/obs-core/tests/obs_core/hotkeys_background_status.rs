//! Tier 1: `obs_core::hotkeys_background_status`, the safe core of
//! `obs_hotkeys_background_status_changed()`.
//!
//! The `characterization_*` cases port every assertion of the unchanged
//! `test/cmocka/test_hotkeys_background_status_changed.c` onto a recording
//! host, its three cases one for one: no core or no handler is silent; each
//! call reaches the handler current at that call (twice the first, then a
//! replacement, then none); and the synchronous callback may re-enter after
//! replacing the handler, or remove the core, and the next call sees it.
//! Each recorded signal checks the exact name and that no more than the
//! C fixture's four destinations are recorded. The trait carries no
//! calldata, so the C's NULL is structural here; Tier 3 and the Tier 2 twin
//! check the NULL through the real adapter.
//!
//! The `edge_*` cases are new: the name's exact bytes, one handler read per
//! call and always before its signal (nothing cached across calls or
//! re-entries), a chain of nested re-entries each reaching the handler of
//! its own moment, and a proptest over any core and handler state and
//! callback against a model of the C.

use core::ffi::CStr;
use obs_core::hotkeys_background_status::{Host, SIGNAL, status_changed};
use proptest::prelude::*;

const NAME: &str = "hotkey_background_status_changed";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Read,
    Signal(u32),
}

/// What the C fixture's `record_signal` does once it has recorded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Callback {
    #[default]
    Nothing,
    /// Install this handler, then notify again.
    Reenter(u32),
    /// Clear the core.
    RemoveCore,
}

/// The fixture core: `current_core` and `core.hotkeys.signals`, handlers
/// named by number.
#[derive(Debug, Default)]
struct Fixture {
    core: bool,
    signals: Option<u32>,
    destinations: Vec<u32>,
    events: Vec<Event>,
    /// Callbacks to run, one per signal, in order; then `Nothing`.
    callbacks: Vec<Callback>,
}

impl Host for Fixture {
    type Handler = u32;
    fn signals(&mut self) -> Option<u32> {
        self.events.push(Event::Read);
        if self.core { self.signals } else { None }
    }
    fn signal(&mut self, handler: u32, name: &CStr) {
        assert!(self.destinations.len() < 4, "the C fixture records four");
        assert_eq!(name.to_str().unwrap(), NAME);
        self.events.push(Event::Signal(handler));
        self.destinations.push(handler);
        let callback = if self.callbacks.is_empty() {
            Callback::Nothing
        } else {
            self.callbacks.remove(0)
        };
        match callback {
            Callback::Nothing => {}
            Callback::RemoveCore => self.core = false,
            Callback::Reenter(next) => {
                self.signals = Some(next);
                status_changed(self);
            }
        }
    }
}

const FIRST: u32 = 1;
const SECOND: u32 = 2;

#[test]
fn characterization_missing_core_or_handler_is_silent() {
    let mut fixture = Fixture::default();
    status_changed(&mut fixture);
    assert!(fixture.destinations.is_empty());
    fixture.core = true;
    status_changed(&mut fixture);
    assert!(fixture.destinations.is_empty());
    assert_eq!(fixture.signals, None);
}

#[test]
fn characterization_notification_uses_current_handler_each_time() {
    let mut fixture = Fixture {
        core: true,
        signals: Some(FIRST),
        ..Fixture::default()
    };
    status_changed(&mut fixture);
    status_changed(&mut fixture);
    assert_eq!(fixture.destinations, [FIRST, FIRST]);
    assert!(fixture.core);
    assert_eq!(fixture.signals, Some(FIRST));

    fixture.signals = Some(SECOND);
    status_changed(&mut fixture);
    assert_eq!(fixture.destinations, [FIRST, FIRST, SECOND]);
    fixture.signals = None;
    status_changed(&mut fixture);
    assert_eq!(fixture.destinations.len(), 3);
}

#[test]
fn characterization_synchronous_callback_can_reenter_or_remove_core() {
    let mut fixture = Fixture {
        core: true,
        signals: Some(FIRST),
        callbacks: vec![Callback::Reenter(SECOND)],
        ..Fixture::default()
    };
    status_changed(&mut fixture);
    assert_eq!(fixture.destinations, [FIRST, SECOND]);
    assert!(fixture.callbacks.is_empty(), "the re-entry ran once");
    assert_eq!(fixture.signals, Some(SECOND));

    fixture.callbacks = vec![Callback::RemoveCore];
    status_changed(&mut fixture);
    assert_eq!(fixture.destinations, [FIRST, SECOND, SECOND]);
    assert!(!fixture.core);
    status_changed(&mut fixture);
    assert_eq!(fixture.destinations.len(), 3);
}

#[test]
fn edge_signal_name_is_exact() {
    assert_eq!(SIGNAL.to_bytes(), NAME.as_bytes());
    assert_eq!(SIGNAL.to_bytes_with_nul().last(), Some(&0));
}

#[test]
fn edge_one_read_per_call_before_its_signal() {
    let mut fixture = Fixture::default();
    status_changed(&mut fixture);
    assert_eq!(fixture.events, [Event::Read]);

    let mut fixture = Fixture {
        core: true,
        signals: Some(FIRST),
        ..Fixture::default()
    };
    status_changed(&mut fixture);
    status_changed(&mut fixture);
    assert_eq!(
        fixture.events,
        [
            Event::Read,
            Event::Signal(FIRST),
            Event::Read,
            Event::Signal(FIRST)
        ]
    );

    // A re-entry reads again; the outer call reads nothing after its signal.
    let mut fixture = Fixture {
        core: true,
        signals: Some(FIRST),
        callbacks: vec![Callback::Reenter(SECOND), Callback::RemoveCore],
        ..Fixture::default()
    };
    status_changed(&mut fixture);
    assert_eq!(
        fixture.events,
        [
            Event::Read,
            Event::Signal(FIRST),
            Event::Read,
            Event::Signal(SECOND)
        ]
    );
    assert!(!fixture.core);
}

#[test]
fn edge_nested_reentries_reach_the_handler_of_their_moment() {
    let mut fixture = Fixture {
        core: true,
        signals: Some(1),
        callbacks: vec![
            Callback::Reenter(2),
            Callback::Reenter(3),
            Callback::Reenter(4),
        ],
        ..Fixture::default()
    };
    status_changed(&mut fixture);
    assert_eq!(fixture.destinations, [1, 2, 3, 4]);
    assert_eq!(fixture.events.len(), 8);
}

#[derive(Clone, Copy, Debug)]
struct Step {
    core: bool,
    signals: Option<u32>,
    callback: Callback,
}

fn step() -> impl Strategy<Value = Step> {
    (
        any::<bool>(),
        prop::option::of(1..5u32),
        prop_oneof![
            4 => Just(Callback::Nothing),
            1 => Just(Callback::RemoveCore),
            1 => (1..5u32).prop_map(Callback::Reenter),
        ],
    )
        .prop_map(|(core, signals, callback)| Step {
            core,
            signals,
            callback,
        })
}

proptest! {
    #[test]
    fn edge_any_state_matches_the_c_model(steps in prop::collection::vec(step(), 0..12)) {
        for s in steps {
            let mut fixture = Fixture {
                core: s.core,
                signals: s.signals,
                callbacks: vec![s.callback],
                ..Fixture::default()
            };
            status_changed(&mut fixture);
            // The C model: no core or no handler, one read and nothing else;
            // otherwise the handler of that moment, then what the callback
            // did (a re-entry notifies the new handler while the core lives).
            let mut expected = vec![Event::Read];
            if let (true, Some(handler)) = (s.core, s.signals) {
                expected.push(Event::Signal(handler));
                if let Callback::Reenter(next) = s.callback {
                    expected.push(Event::Read);
                    expected.push(Event::Signal(next));
                }
            }
            prop_assert_eq!(&fixture.events, &expected);
            let removed = s.core && s.signals.is_some() && s.callback == Callback::RemoveCore;
            prop_assert_eq!(fixture.core, s.core && !removed);
        }
    }
}
