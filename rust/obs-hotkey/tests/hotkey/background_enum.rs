//! Tier 1 for `obs_hotkeys_enum_background_shortcuts`: a 1:1 port of
//! test/cmocka/test_hotkey_background_enum.c over a model list, plus edges.
use obs_hotkey::background_enum::{BackgroundShortcuts, enumerate};
use std::cell::RefCell;
use std::rc::Rc;

const UINT32_MAX: u32 = u32::MAX;
const OBS_KEY_NONE: i32 = 0;
const INTERACT_SHIFT_KEY: u32 = 1 << 1;
const INTERACT_CONTROL_KEY: u32 = 1 << 2;
const INTERACT_ALT_KEY: u32 = 1 << 3;
// Arbitrary distinct key values: the enumeration passes them through.
const OBS_KEY_A: i32 = 65;
const OBS_KEY_B: i32 = 66;
const OBS_KEY_C: i32 = 67;
const OBS_KEY_MOUSE3: i32 = 300;

/// The model of `struct obs_hotkey_background_shortcut`; the trigger is a
/// borrowed pointer in C, so identity is compared with `ptr::eq`.
#[derive(Clone, Copy, Debug)]
struct Shortcut {
    modifiers: u32,
    key: i32,
    trigger: Option<&'static str>,
    bound: bool,
}

const fn shortcut(
    modifiers: u32,
    key: i32,
    trigger: Option<&'static str>,
    bound: bool,
) -> Shortcut {
    Shortcut {
        modifiers,
        key,
        trigger,
        bound,
    }
}

#[derive(Default)]
struct State {
    available: bool,
    held: bool,
    shortcuts: Vec<Shortcut>,
    locks: usize,
    unlocks: usize,
}

/// The core's mutex and live list, shared with callbacks like `core` in C.
#[derive(Clone, Default)]
struct Core(Rc<RefCell<State>>);

impl Core {
    fn new(available: bool, shortcuts: Vec<Shortcut>) -> Self {
        Self(Rc::new(RefCell::new(State {
            available,
            shortcuts,
            ..State::default()
        })))
    }
    fn held(&self) -> bool {
        self.0.borrow().held
    }
}

impl BackgroundShortcuts for Core {
    type Shortcut = Shortcut;
    fn lock(&mut self) -> bool {
        let mut state = self.0.borrow_mut();
        // A nonrecursive mutex already held by this thread would deadlock.
        assert!(!state.held, "lock while held");
        state.locks += 1;
        state.held = state.available;
        state.available
    }
    fn count(&self) -> usize {
        let state = self.0.borrow();
        assert!(state.held);
        state.shortcuts.len()
    }
    fn get(&self, index: usize) -> Shortcut {
        let state = self.0.borrow();
        assert!(state.held);
        state.shortcuts[index]
    }
    fn unlock(&mut self) {
        let mut state = self.0.borrow_mut();
        assert!(state.held);
        state.unlocks += 1;
        state.held = false;
    }
}

fn same_trigger(a: Option<&'static str>, b: Option<&'static str>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => std::ptr::eq(a, b),
        _ => false,
    }
}

/// `struct enumeration` and `collect()` of the C test.
fn collect(
    core: &Core,
    expected: &[Shortcut],
    stop_after: usize,
    change_live_list: Option<Vec<Shortcut>>,
) -> usize {
    let mut calls = 0;
    let watched = core.clone();
    let mut list = core.clone();
    let mut change = change_live_list;
    enumerate(
        &mut list,
        Some(|combo: Shortcut| {
            let index = calls;
            calls += 1;
            assert!(index < watched.0.borrow().shortcuts.len());
            let want = expected[index];
            assert_eq!(combo.modifiers, want.modifiers);
            assert_eq!(combo.key, want.key);
            assert!(same_trigger(combo.trigger, want.trigger));
            assert_eq!(combo.bound, want.bound);
            // expect_mutex(EBUSY)
            assert!(watched.held());
            if index == 0
                && let Some(next) = change.take()
            {
                // Characterized, not endorsed: later entries and num are
                // read live rather than snapshotted before the callbacks.
                watched.0.borrow_mut().shortcuts = next;
            }
            calls < stop_after
        }),
    );
    calls
}

fn no_callback() -> Option<fn(Shortcut) -> bool> {
    None
}

/// Mirrors `no_core_and_empty_list` in test_hotkey_background_enum.c.
#[test]
fn test_hotkey_background_enum_no_core_and_empty_list() {
    let mut core = Core::new(false, vec![]);
    enumerate(&mut core, no_callback());
    assert_eq!(collect(&core, &[], 99, None), 0);
    core.0.borrow_mut().available = true;
    assert_eq!(collect(&core, &[], 99, None), 0);
    // expect_mutex(0)
    assert!(!core.held());
    let state = core.0.borrow();
    // The NULL callback never locked; the unavailable core locked once and
    // was not unlocked; the empty list locked and unlocked once.
    assert_eq!((state.locks, state.unlocks), (2, 1));
}

/// Mirrors `null_callback_does_not_lock`.
#[test]
fn test_hotkey_background_enum_null_callback_does_not_lock() {
    let mut core = Core::new(true, vec![]);
    // Hold the mutex: the model panics if enumerate locks again.
    assert!(core.lock());
    enumerate(&mut core, no_callback());
    core.unlock();
    assert!(!core.held());
    assert_eq!(core.0.borrow().locks, 1);
}

/// Mirrors `ordered_arguments_and_early_stop`.
#[test]
fn test_hotkey_background_enum_ordered_arguments_and_early_stop() {
    let bytes: &'static str = Box::leak(String::from("x\u{ff}").into_boxed_str());
    let empty: &'static str = Box::leak(String::new().into_boxed_str());
    let shortcuts = vec![
        shortcut(UINT32_MAX, OBS_KEY_NONE, None, false),
        shortcut(INTERACT_CONTROL_KEY, OBS_KEY_A, Some(bytes), true),
        shortcut(INTERACT_CONTROL_KEY, OBS_KEY_A, Some(empty), false),
        shortcut(0, OBS_KEY_MOUSE3, Some(bytes), true),
    ];
    let core = Core::new(true, shortcuts.clone());
    for stop in 1..=5 {
        let calls = collect(&core, &shortcuts, stop, None);
        assert_eq!(calls, stop.min(4));
        assert!(!core.held());
        let state = core.0.borrow();
        assert_eq!(state.shortcuts.len(), 4);
        assert_eq!(state.locks, state.unlocks);
        assert!(std::ptr::eq(state.shortcuts[1].trigger.unwrap(), bytes));
        assert!(bytes.starts_with('x'));
    }
}

/// Mirrors `list_is_read_live`.
#[test]
fn test_hotkey_background_enum_list_is_read_live() {
    let first: &'static str = "first";
    let second: &'static str = "second";
    let initial = vec![shortcut(0, OBS_KEY_A, Some(first), false)];
    let replacement = vec![
        shortcut(0, OBS_KEY_A, Some(first), false),
        shortcut(INTERACT_ALT_KEY, OBS_KEY_B, Some(second), true),
        shortcut(INTERACT_SHIFT_KEY, OBS_KEY_C, None, false),
    ];
    let core = Core::new(true, initial);
    let calls = collect(&core, &replacement, 99, Some(replacement.clone()));
    assert_eq!(calls, 3);
    assert!(!core.held());
}

/// A callback that shrinks the list stops the walk at the new count.
#[test]
fn shrinking_list_stops_at_live_count() {
    let entries = vec![
        shortcut(0, OBS_KEY_A, None, false),
        shortcut(0, OBS_KEY_B, None, true),
        shortcut(0, OBS_KEY_C, None, false),
    ];
    let core = Core::new(true, entries.clone());
    let calls = collect(&core, &entries, 99, Some(entries[..1].to_vec()));
    assert_eq!(calls, 1);
    assert!(!core.held());
}

/// Stopping on the first entry still unlocks exactly once, and a list that
/// is never stopped visits every entry once.
#[test]
fn every_acquired_lock_is_released_once() {
    let entries: Vec<_> = (0..6)
        .map(|key| shortcut(0, key, None, key % 2 == 0))
        .collect();
    for stop in [0, 1, 6, 7] {
        let core = Core::new(true, entries.clone());
        let calls = collect(&core, &entries, stop, None);
        assert_eq!(calls, stop.clamp(1, 6));
        let state = core.0.borrow();
        assert_eq!((state.locks, state.unlocks, state.held), (1, 1, false));
    }
}
