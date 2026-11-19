//! Tier 1: mirror test_source_update_properties.c using safe borrowed tokens.
use obs_source::update_properties::{Source, update_properties};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Validate,
    Log(u32, &'static str),
    Signal(Option<String>, String, usize),
}

struct Probe {
    present: bool,
    _private: bool,
    token: usize,
    settings: String,
    events: RefCell<Vec<Event>>,
}

impl Source for Probe {
    fn valid(&self) -> bool {
        self.events.borrow_mut().push(Event::Validate);
        if !self.present {
            self.events.borrow_mut().push(Event::Log(
                400,
                "obs_source_update_properties: Null 'source' parameter",
            ));
        }
        self.present
    }

    fn signal_update_properties(&self) {
        assert!(self.present);
        self.events.borrow_mut().push(Event::Signal(
            None, "update_properties".into(), self.token,
        ));
    }

}

fn probe(present: bool, private: bool) -> Probe {
    Probe {
        present, _private: private, token: 0x5204, settings: "{\"marker\":\"kept\"}".into(),
        events: RefCell::new(Vec::new()),
    }
}

#[test]
fn test_source_update_properties_null_source() {
    let source = probe(false, false);
    update_properties(&source);
    assert_eq!(*source.events.borrow(), [Event::Validate, Event::Log(
        400, "obs_source_update_properties: Null 'source' parameter",
    )]);
}

fn signals_source_only(private: bool) {
    let source = probe(true, private);
    for count in 1..=3 {
        update_properties(&source);
        assert_eq!(source.events.borrow().len(), count * 2);
        assert_eq!(&source.events.borrow()[(count - 1) * 2..], [
            Event::Validate, Event::Signal(None, "update_properties".into(), source.token),
        ]);
    }
}

#[test]
fn test_source_update_properties_public_source() {
    signals_source_only(false);
}

#[test]
fn test_source_update_properties_private_source() {
    signals_source_only(true);
}

#[test]
fn test_source_update_properties_settings_untouched() {
    let source = probe(true, true);
    let before = source.settings.clone();
    update_properties(&source);
    assert_eq!(source.settings, before);
}
