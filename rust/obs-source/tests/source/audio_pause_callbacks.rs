//! Tier 1: mirrors test_source_audio_pause_callbacks.c without FFI.
use obs_source::audio_pause_callbacks::update;
use std::cell::RefCell;

#[derive(Default)]
struct Signals {
    subscriptions: Vec<(&'static core::ffi::CStr, usize)>,
    calls: [usize; 2],
}
impl Signals {
    fn change(&mut self, adding: bool, param: usize) {
        update(
            true,
            adding,
            || (),
            |(), name| {
                if adding {
                    if !self.subscriptions.contains(&(name, param)) {
                        self.subscriptions.push((name, param));
                    }
                } else {
                    self.subscriptions.retain(|entry| *entry != (name, param));
                }
            },
            |_| panic!("valid source"),
        );
    }
    fn emit(&mut self, name: &core::ffi::CStr) {
        for &(signal, param) in &self.subscriptions {
            if signal == name {
                self.calls[param] += 1;
            }
        }
    }
}

#[test]
fn subscriptions_and_removal_from_c() {
    let mut signals = Signals::default();
    signals.change(true, 0);
    signals.change(true, 1);
    signals.emit(c"media_pause");
    assert_eq!(signals.calls, [1, 1]);
    signals.emit(c"media_stopped");
    assert_eq!(signals.calls, [2, 2]);
    signals.change(false, 0);
    signals.emit(c"media_pause");
    assert_eq!(signals.calls, [2, 3]);
    signals.emit(c"media_stopped");
    assert_eq!(signals.calls, [2, 4]);
    signals.change(false, 1);
    assert!(signals.subscriptions.is_empty());
}

#[test]
fn null_source_from_c() {
    for (adding, name) in [
        (true, c"obs_source_add_audio_pause_callback"),
        (false, c"obs_source_remove_audio_pause_callback"),
    ] {
        let logs = RefCell::new(Vec::new());
        update(
            false,
            adding,
            || panic!("must not acquire handler"),
            |_: &(), _| panic!("must not invoke callback"),
            |name| logs.borrow_mut().push(name),
        );
        assert_eq!(*logs.borrow(), [name]);
    }
}

#[test]
fn acquires_handler_once_before_paired_updates() {
    let events = RefCell::new(Vec::new());
    update(
        true,
        true,
        || {
            events.borrow_mut().push("get");
            42
        },
        |handler, name| {
            assert_eq!(*handler, 42);
            events.borrow_mut().push(name.to_str().unwrap());
        },
        |_| panic!("valid source"),
    );
    assert_eq!(*events.borrow(), ["get", "media_pause", "media_stopped"]);
}
