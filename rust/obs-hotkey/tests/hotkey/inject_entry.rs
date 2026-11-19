use obs_hotkey::inject_entry::{Event, Registry, forward, inject};
use obs_hotkey::name_map::{KEY_NONE, OBS_KEY_A, OBS_KEY_MOUSE3};
use obs_hotkey::nix_hotkey_state::INTERACT_CONTROL_KEY;
use obs_hotkey::press::KeyCombination;
use std::cell::Cell;

#[derive(Default)]
pub struct Recorder {
    pub present: bool,
    pub strict_at_lock: bool,
    pub count: usize,
    pub locked: bool,
    pub strict: Cell<bool>,
    pub order: Vec<u8>,
    pub visits: Vec<(usize, Event)>,
    pub locks: usize,
    pub enumerations: usize,
    pub unlocks: usize,
}

impl Registry for Recorder {
    fn lock(&mut self) -> bool {
        assert!(!self.locked);
        self.locks += 1;
        self.order.push(b'L');
        self.locked = self.present;
        if self.locked {
            self.strict.set(self.strict_at_lock);
        }
        self.locked
    }
    fn strict_modifiers(&self) -> bool {
        assert!(self.locked);
        self.strict.get()
    }
    fn bindings(&mut self, event: Event) {
        assert!(self.locked);
        self.enumerations += 1;
        self.order.push(b'E');
        self.strict.set(!self.strict_at_lock);
        for index in 0..self.count {
            assert!(forward(event, index, |binding, event| {
                assert!(self.locked);
                assert_eq!(binding, self.visits.len());
                assert_eq!(event.strict_modifiers, self.strict_at_lock);
                self.visits.push((binding, event));
                self.order.push(b'B');
                self.strict.set(!self.strict.get());
            }));
        }
    }
    fn unlock(&mut self) {
        assert!(self.locked);
        assert_eq!(self.visits.len(), self.count);
        self.unlocks += 1;
        self.order.push(b'U');
        self.locked = false;
    }
}

#[test]
fn mirrors_test_hotkey_inject_entry_no_core() {
    let mut r = Recorder::default();
    inject(
        &mut r,
        KeyCombination {
            modifiers: u32::MAX,
            key: OBS_KEY_MOUSE3,
        },
        false,
    );
    assert_eq!(
        (r.locks, r.enumerations, r.unlocks, r.visits.len()),
        (1, 0, 0, 0)
    );
    assert_eq!(r.order, b"L");
    assert!(!r.locked);
}

#[test]
fn mirrors_test_hotkey_inject_entry_empty_registry() {
    let mut r = Recorder {
        present: true,
        strict_at_lock: true,
        ..Recorder::default()
    };
    inject(
        &mut r,
        KeyCombination {
            modifiers: 0,
            key: KEY_NONE,
        },
        true,
    );
    assert_eq!((r.locks, r.enumerations, r.unlocks), (1, 1, 1));
    assert_eq!(r.order, b"LEU");
    assert!(!r.locked);
}

#[test]
fn mirrors_test_hotkey_inject_entry_forwarding() {
    let mut r = Recorder {
        present: true,
        count: 3,
        ..Recorder::default()
    };
    for hotkey in [
        KeyCombination {
            modifiers: 0,
            key: KEY_NONE,
        },
        KeyCombination {
            modifiers: u32::MAX,
            key: OBS_KEY_MOUSE3,
        },
        KeyCombination {
            modifiers: INTERACT_CONTROL_KEY,
            key: OBS_KEY_A,
        },
    ] {
        for flags in 0..4 {
            let pressed = flags & 1 != 0;
            r.strict_at_lock = flags & 2 != 0;
            r.strict.set(!r.strict_at_lock);
            r.order.clear();
            r.visits.clear();
            inject(&mut r, hotkey, pressed);
            assert_eq!(
                r.visits,
                (0..3)
                    .map(|i| (
                        i,
                        Event {
                            hotkey,
                            pressed,
                            strict_modifiers: r.strict_at_lock
                        }
                    ))
                    .collect::<Vec<_>>()
            );
            assert_eq!(r.order, b"LEBBBU");
            assert!(!r.locked);
        }
    }
    assert_eq!((r.locks, r.enumerations, r.unlocks), (12, 12, 12));
}
