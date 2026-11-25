//! Tier 1 mirrors of test_source_defaults.c.
use obs_source::defaults::{Callback, callback, defaults};
use std::cell::RefCell;

/// A model settings object: which one it is, and the defaults written to it.
#[derive(Debug, PartialEq)]
struct Settings {
    id: u32,
    which: Option<i64>,
}

#[derive(Debug, PartialEq)]
enum Event {
    Create(u32),
    Legacy(u32),
    Modern(u32),
}

struct Run {
    settings: Option<RefCell<Settings>>,
    events: Vec<Event>,
}

/// One lookup: `found` is `None` for an unknown ID, otherwise which of the
/// two callbacks the type registered (`(modern, legacy)`).
fn run(found: Option<(bool, bool)>, id: u32) -> Run {
    let events = RefCell::new(Vec::new());
    let settings = defaults(
        found.map(|(modern, legacy)| callback(modern, legacy)),
        || {
            events.borrow_mut().push(Event::Create(id));
            RefCell::new(Settings { id, which: None })
        },
        |kind, settings: &RefCell<Settings>| {
            let mut settings = settings.borrow_mut();
            // The callback sees the new, still empty object.
            assert_eq!(settings.which, None);
            match kind {
                Callback::Modern => {
                    events.borrow_mut().push(Event::Modern(settings.id));
                    settings.which = Some(2);
                }
                Callback::Legacy => {
                    events.borrow_mut().push(Event::Legacy(settings.id));
                    settings.which = Some(1);
                }
                Callback::Neither => panic!("no callback to run"),
            }
        },
    );
    Run {
        settings,
        events: events.into_inner(),
    }
}

#[test]
fn test_source_defaults_unknown_id_returns_null() {
    let got = run(None, 1);
    assert!(got.settings.is_none());
    assert!(got.events.is_empty());
}

#[test]
fn test_source_defaults_legacy_callback_fills_returned_object() {
    let got = run(Some((false, true)), 1);
    assert_eq!(got.events, [Event::Create(1), Event::Legacy(1)]);
    let settings = got.settings.unwrap().into_inner();
    assert_eq!(
        settings,
        Settings {
            id: 1,
            which: Some(1)
        }
    );
}

#[test]
fn test_source_defaults_modern_callback_gets_type_data() {
    let got = run(Some((true, false)), 1);
    assert_eq!(got.events, [Event::Create(1), Event::Modern(1)]);
    let settings = got.settings.unwrap().into_inner();
    assert_eq!(
        settings,
        Settings {
            id: 1,
            which: Some(2)
        }
    );
}

#[test]
fn test_source_defaults_modern_callback_wins() {
    assert_eq!(callback(true, true), Callback::Modern);
    let got = run(Some((true, true)), 1);
    assert_eq!(got.events, [Event::Create(1), Event::Modern(1)]);
}

#[test]
fn test_source_defaults_no_callback_returns_empty_object() {
    assert_eq!(callback(false, false), Callback::Neither);
    let got = run(Some((false, false)), 1);
    assert_eq!(got.events, [Event::Create(1)]);
    let settings = got.settings.unwrap().into_inner();
    assert_eq!(settings, Settings { id: 1, which: None });
}

#[test]
fn test_source_defaults_each_call_returns_a_new_object() {
    let first = run(Some((false, true)), 1);
    let second = run(Some((false, true)), 2);
    assert_eq!(first.events, [Event::Create(1), Event::Legacy(1)]);
    assert_eq!(second.events, [Event::Create(2), Event::Legacy(2)]);
    assert_eq!(first.settings.unwrap().into_inner().id, 1);
    assert_eq!(second.settings.unwrap().into_inner().id, 2);
}
