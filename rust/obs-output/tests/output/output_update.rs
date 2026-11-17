//! Tier 1 mirrors of test_output_update.c over a model output: the stored
//! and incoming settings are maps, `obs_data_apply()` merges one into the
//! other (a NULL incoming object merges nothing), and the plugin's update
//! callback sees the stored map. Every read and call is recorded in order.
use core::cell::RefCell;
use core::ffi::CStr;
use obs_output::output_update::update;
use std::collections::BTreeMap;

type Settings = BTreeMap<&'static str, i64>;

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Log(&'static CStr),
    Apply,
    HasUpdate,
    Update(Settings),
}

struct Output {
    settings: Settings,
    has_update: bool,
}

fn run(output: Option<&mut Output>, incoming: Option<&Settings>) -> Vec<Event> {
    let events = RefCell::new(Vec::new());
    let valid = output.is_some();
    let output = RefCell::new(output);
    update(
        valid,
        || {
            events.borrow_mut().push(Event::Apply);
            let mut output = output.borrow_mut();
            let output = output.as_mut().expect("applied to a NULL output");
            if let Some(incoming) = incoming {
                output
                    .settings
                    .extend(incoming.iter().map(|(k, v)| (*k, *v)));
            }
        },
        || {
            events.borrow_mut().push(Event::HasUpdate);
            output
                .borrow()
                .as_ref()
                .expect("read a NULL output")
                .has_update
        },
        || {
            let output = output.borrow();
            let output = output.as_ref().expect("called a NULL output");
            assert!(output.has_update, "called a missing update");
            events
                .borrow_mut()
                .push(Event::Update(output.settings.clone()));
        },
        |name| events.borrow_mut().push(Event::Log(name)),
    );
    events.into_inner()
}

fn settings(pairs: &[(&'static str, i64)]) -> Settings {
    pairs.iter().copied().collect()
}

#[test]
fn merge_before_callback() {
    let mut output = Output {
        settings: settings(&[("overwrite", 10), ("preserved", 1)]),
        has_update: true,
    };
    let mut incoming = settings(&[("overwrite", 20), ("added", 1)]);
    let merged = settings(&[("overwrite", 20), ("preserved", 1), ("added", 1)]);
    assert_eq!(
        run(Some(&mut output), Some(&incoming)),
        [
            Event::Apply,
            Event::HasUpdate,
            Event::Update(merged.clone())
        ]
    );
    assert_eq!(output.settings, merged);
    assert!(!incoming.contains_key("preserved"));

    incoming.insert("overwrite", 30);
    let merged = settings(&[("overwrite", 30), ("preserved", 1), ("added", 1)]);
    assert_eq!(
        run(Some(&mut output), Some(&incoming)),
        [
            Event::Apply,
            Event::HasUpdate,
            Event::Update(merged.clone())
        ]
    );
    // Applying copies values: a later change to the incoming object is not
    // seen by the stored settings.
    incoming.insert("overwrite", 40);
    assert_eq!(output.settings, merged);
}

#[test]
fn merge_without_callback() {
    let mut output = Output {
        settings: settings(&[("overwrite", 1), ("preserved", 1)]),
        has_update: false,
    };
    let incoming = settings(&[("overwrite", 2), ("added", 1)]);
    assert_eq!(
        run(Some(&mut output), Some(&incoming)),
        [Event::Apply, Event::HasUpdate]
    );
    assert_eq!(
        output.settings,
        settings(&[("overwrite", 2), ("preserved", 1), ("added", 1)])
    );
}

#[test]
fn null_settings_still_call_update() {
    // Characterized, not endorsed: NULL incoming settings merge nothing, and
    // the callback still runs.
    let stored = settings(&[("overwrite", 7), ("preserved", 1), ("added", 1)]);
    let mut output = Output {
        settings: stored.clone(),
        has_update: true,
    };
    assert_eq!(
        run(Some(&mut output), None),
        [
            Event::Apply,
            Event::HasUpdate,
            Event::Update(stored.clone())
        ]
    );
    assert_eq!(output.settings, stored);
}

#[test]
fn null_output() {
    assert_eq!(
        run(None, Some(&settings(&[("overwrite", 19)]))),
        [Event::Log(c"obs_output_update")]
    );
}
