//! Tier 1 setter state and synchronous signal contract.
use obs_source::enabled_setter::{Source, set_enabled};
use std::cell::{Cell, RefCell};

#[derive(Default)]
struct Fake {
    enabled: Cell<bool>,
    events: RefCell<Vec<(bool, bool)>>,
}

impl Source for Fake {
    fn store_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
    }

    fn signal_enable(&self, enabled: bool) {
        self.events.borrow_mut().push((enabled, self.enabled.get()));
    }
}

#[test]
fn test_source_audio_output_disabled_source_flag() {
    // test_source_audio_output.c's disabled monitor-only capture requires this
    // stored flag; its packet/capture assertions remain in audio_output tests
    // and the unchanged C suite, rather than duplicating audio here.
    let source = Fake::default();
    source.enabled.set(true);
    set_enabled(Some(&source), false, || panic!("valid source"));
    assert!(!source.enabled.get());
}

#[test]
fn stores_before_signal_including_repeated_values() {
    let source = Fake::default();
    for enabled in [false, true, true, false, false] {
        set_enabled(Some(&source), enabled, || panic!("valid source"));
        assert_eq!(source.enabled.get(), enabled);
    }
    assert_eq!(*source.events.borrow(), [(false, false), (true, true), (true, true), (false, false), (false, false)]);
}

#[test]
fn null_logs_once_without_touching_source() {
    for enabled in [false, true] {
        let logs = Cell::new(0);
        set_enabled(None::<&Fake>, enabled, || logs.set(logs.get() + 1));
        assert_eq!(logs.get(), 1);
    }
}
