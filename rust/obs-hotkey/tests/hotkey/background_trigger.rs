//! Tier 1: mirrors test_hotkey_background_trigger.c without FFI (#5104).
use std::cell::Cell;
use std::rc::Rc;

use obs_hotkey::background_trigger::{BackgroundTrigger, query};
use obs_hotkey::nix_hotkey_state::KeyCombination;

struct Storage {
    allocation: Option<Vec<u8>>,
    live: Rc<Cell<usize>>,
    held: Rc<Cell<bool>>,
}
impl Drop for Storage {
    fn drop(&mut self) {
        assert!(!self.held.get(), "cleanup follows unlock");
        if self.allocation.is_some() {
            self.live.set(self.live.get() - 1);
        }
    }
}

#[derive(Default)]
struct Background {
    available: bool,
    callback: bool,
    found: bool,
    text: Option<&'static [u8]>,
    calls: usize,
    locks: usize,
    unlocks: usize,
    held: Rc<Cell<bool>>,
    live: Rc<Cell<usize>>,
    expected: KeyCombination,
}
impl BackgroundTrigger for Background {
    type Trigger = Storage;
    fn lock(&mut self) -> bool {
        assert!(!self.held.get());
        self.locks += 1;
        self.held.set(self.available);
        self.available
    }
    fn trigger(&mut self, combo: KeyCombination) -> (bool, Storage) {
        assert!(self.held.get());
        let mut storage = Storage {
            allocation: None,
            live: self.live.clone(),
            held: self.held.clone(),
        };
        if !self.callback {
            return (false, storage);
        }
        assert_eq!(combo, self.expected);
        assert!(
            storage.allocation.is_none(),
            "callback starts with empty storage"
        );
        self.calls += 1;
        if let Some(text) = self.text {
            storage.allocation = Some(text.to_vec());
            self.live.set(self.live.get() + 1);
        }
        (self.found, storage)
    }
    fn unlock(&mut self) {
        assert!(self.held.replace(false));
        self.unlocks += 1;
    }
}

#[test]
fn background_trigger_characterization() {
    let mut background = Background {
        expected: KeyCombination {
            modifiers: u32::MAX,
            key: 1,
        },
        ..Background::default()
    };
    assert!(
        query(
            &mut background,
            KeyCombination {
                modifiers: u32::MAX,
                key: 1
            }
        )
        .is_none()
    );
    assert_eq!((background.calls, background.unlocks), (0, 0));
    background.available = true;
    assert!(query(&mut background, background_combo()).is_none());
    assert_eq!((background.calls, background.unlocks), (0, 1));
    background.callback = true;
    assert!(query(&mut background, background_combo()).is_none());
    assert_eq!(background.calls, 1);
    background.text = Some(b"allocated but rejected");
    assert!(query(&mut background, background_combo()).is_none());
    assert_eq!(background.live.get(), 0);
    assert_eq!(background.calls, 2);
    background.found = true;
    background.text = Some(b"Ctrl+Alt+Shift+A (owned trigger)");
    let result = query(&mut background, background_combo()).unwrap();
    assert_eq!(result.allocation.as_deref(), background.text);
    assert_eq!(background.live.get(), 1);
    drop(result);
    assert_eq!(background.live.get(), 0);
    background.expected = KeyCombination::default();
    background.text = Some(b"");
    let result = query(&mut background, KeyCombination::default()).unwrap();
    assert_eq!(result.allocation.as_deref(), Some(b"".as_slice()));
    drop(result);
    assert_eq!(background.live.get(), 0);
    background.text = None;
    let result = query(&mut background, KeyCombination::default()).unwrap();
    assert!(result.allocation.is_none());
    assert_eq!(background.calls, 5);
    background.available = false;
    assert!(query(&mut background, KeyCombination::default()).is_none());
    assert_eq!(background.calls, 5);
    assert_eq!((background.locks, background.unlocks), (8, 6));
    assert!(!background.held.get());
}

fn background_combo() -> KeyCombination {
    KeyCombination {
        modifiers: u32::MAX,
        key: 1,
    }
}
