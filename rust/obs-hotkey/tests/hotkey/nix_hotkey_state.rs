//! Tier 1: the safe core of `libobs/obs-nix-hotkey-state.c.h`.
//!
//! The first eight tests mirror `test/cmocka/test_wayland_hotkeys.c` one to
//! one, with the same names. The C test runs the real libobs hotkey thread;
//! here [`Thread::cycle`] is one poll cycle of it, as `query_hotkeys` and
//! `handle_binding` in `obs-hotkey.c.h` run it with strict modifiers (the
//! test's setting): the lock is held for the whole cycle, the four modifier
//! keys are queried first, and a binding whose key and exact modifiers are
//! reported is pressed, one that is not is released. [`Thread::inject`] is
//! the press Qt injects for a focused window.
//!
//! The cases after those pin the edges of each function.

use std::cell::Cell;

use obs_hotkey::name_map::key_from_name;
use obs_hotkey::nix_hotkey_state::{
    self as hs, HotkeyLock, INTERACT_ALT_KEY, INTERACT_COMMAND_KEY, INTERACT_CONTROL_KEY,
    INTERACT_SHIFT_KEY, KeyCombination, KeyState, NoLock, OBS_KEY_LAST_VALUE, Shortcut,
};

/// Counts lock and unlock calls and checks they nest.
#[derive(Default)]
struct CountingLock {
    depth: Cell<u32>,
    locks: Cell<u32>,
}

impl HotkeyLock for CountingLock {
    fn lock(&self) {
        self.depth.set(self.depth.get() + 1);
        self.locks.set(self.locks.get() + 1);
    }
    fn unlock(&self) {
        let depth = self.depth.get();
        assert!(depth > 0, "unlock without lock");
        self.depth.set(depth - 1);
    }
}

impl CountingLock {
    /// Lock calls since the last `take`, all of them released.
    fn take(&self) -> u32 {
        assert_eq!(self.depth.get(), 0, "lock still held");
        self.locks.replace(0)
    }
}

fn key(name: &str) -> i32 {
    let name = std::ffi::CString::new(name).unwrap();
    let code = key_from_name(&name);
    assert_ne!(code, 0, "{name:?} is a key");
    code
}

fn f13() -> KeyCombination {
    KeyCombination {
        modifiers: 0,
        key: key("OBS_KEY_F13"),
    }
}

fn ctrl_shift_f13() -> KeyCombination {
    KeyCombination {
        modifiers: INTERACT_CONTROL_KEY | INTERACT_SHIFT_KEY,
        key: key("OBS_KEY_F13"),
    }
}

/// One binding of one hotkey, as `obs_hotkey_binding_t` keeps it.
#[derive(Default)]
struct Binding {
    combo: KeyCombination,
    pressed: bool,
    modifiers_match: bool,
    presses: u32,
    releases: u32,
}

impl Binding {
    fn new(combo: KeyCombination) -> Self {
        Self {
            combo,
            ..Self::default()
        }
    }
    fn press(&mut self) {
        if !self.pressed {
            self.pressed = true;
            self.presses += 1;
        }
    }
    fn release(&mut self) {
        if self.pressed {
            self.pressed = false;
            self.releases += 1;
        }
    }
}

/// The hotkey thread and the state it reads.
#[derive(Default)]
struct Thread {
    state: KeyState,
    lock: CountingLock,
    bindings: Vec<Binding>,
}

impl Thread {
    fn bind(&mut self, combo: KeyCombination) -> usize {
        self.bindings.push(Binding::new(combo));
        self.bindings.len() - 1
    }

    fn pressed(&mut self, key: i32) -> bool {
        hs::is_pressed(&mut self.state, &self.lock, key)
    }

    /// One poll cycle (`query_hotkeys` with strict modifiers).
    fn cycle(&mut self) {
        self.lock.lock();
        let mut modifiers = 0;
        for (name, bit) in [
            ("OBS_KEY_SHIFT", INTERACT_SHIFT_KEY),
            ("OBS_KEY_CONTROL", INTERACT_CONTROL_KEY),
            ("OBS_KEY_ALT", INTERACT_ALT_KEY),
            ("OBS_KEY_META", INTERACT_COMMAND_KEY),
        ] {
            if self.pressed(key(name)) {
                modifiers |= bit;
            }
        }
        for i in 0..self.bindings.len() {
            let combo = self.bindings[i].combo;
            let matches = combo.modifiers == modifiers;
            let down = self.bindings[i].modifiers_match && matches && self.pressed(combo.key);
            let b = &mut self.bindings[i];
            if down {
                b.press();
            } else {
                b.modifiers_match = matches;
                b.release();
            }
        }
        self.lock.unlock();
    }

    /// Several cycles, as the C test's `wait_cycles` (150 ms of 25 ms).
    fn cycles(&mut self) {
        for _ in 0..6 {
            self.cycle();
        }
    }

    /// `obs_hotkey_inject_event(combo, true)`: Qt's press for a focused
    /// window presses every binding of that combination.
    fn inject(&mut self, combo: KeyCombination) {
        for b in &mut self.bindings {
            if b.combo == combo {
                b.modifiers_match = true;
                b.press();
            }
        }
    }
}

/// Push-to-talk on an unfocused OBS: the portal's Activated presses the
/// hotkey, it stays pressed while held, Deactivated releases it.
#[test]
fn portal_shortcut_holds_push_to_talk() {
    let mut t = Thread::default();
    let ptt = t.bind(f13());

    hs::set_shortcut(&mut t.state, &t.lock, f13(), true);
    t.cycles();
    assert!(t.bindings[ptt].pressed, "Activated");
    t.cycles();
    assert!(t.bindings[ptt].pressed);
    assert_eq!(t.bindings[ptt].releases, 0, "held shortcut was released");

    hs::set_shortcut(&mut t.state, &t.lock, f13(), false);
    t.cycles();
    assert!(!t.bindings[ptt].pressed, "Deactivated");
    assert_eq!(t.bindings[ptt].presses, 1);
    assert_eq!(t.bindings[ptt].releases, 1);
}

/// A combination with modifiers: the strict modifier match in the hotkey
/// thread must see the shortcut's modifiers as held too.
#[test]
fn portal_shortcut_with_modifiers() {
    let mut t = Thread::default();
    let combo = t.bind(ctrl_shift_f13());
    let plain = t.bind(f13());

    hs::set_shortcut(&mut t.state, &t.lock, ctrl_shift_f13(), true);
    t.cycles();
    assert!(t.bindings[combo].pressed, "Activated ctrl+shift+F13");
    t.cycles();
    // Strict modifiers: plain F13 is a different combination.
    assert_eq!(t.bindings[plain].presses, 0);

    hs::set_shortcut(&mut t.state, &t.lock, ctrl_shift_f13(), false);
    t.cycles();
    assert!(!t.bindings[combo].pressed, "Deactivated ctrl+shift+F13");
}

/// One shortcut per key combination: every action bound to it fires.
#[test]
fn one_shortcut_drives_every_bound_action() {
    let mut t = Thread::default();
    let talk = t.bind(f13());
    let mute = t.bind(f13());

    hs::set_shortcut(&mut t.state, &t.lock, f13(), true);
    t.cycles();
    assert!(t.bindings[talk].pressed, "push-to-talk");
    assert!(t.bindings[mute].pressed, "push-to-mute");

    hs::set_shortcut(&mut t.state, &t.lock, f13(), false);
    t.cycles();
    assert!(!t.bindings[talk].pressed, "push-to-talk");
    assert!(!t.bindings[mute].pressed, "push-to-mute");
}

/// Activated and Deactivated before the hotkey thread polls: the tap must
/// still press (and then release) the hotkey.
#[test]
fn short_tap_is_not_lost() {
    let mut t = Thread::default();
    let tap = t.bind(f13());
    t.cycles();

    t.lock.lock();
    hs::set_shortcut(&mut t.state, &t.lock, f13(), true);
    hs::set_shortcut(&mut t.state, &t.lock, f13(), false);
    t.lock.unlock();

    for _ in 0..20 {
        if !hs::flush(&mut t.state, &t.lock) {
            break;
        }
        t.cycle();
    }
    t.cycles();

    assert_eq!(t.bindings[tap].presses, 1, "tap presses");
    assert_eq!(t.bindings[tap].releases, 1, "tap releases");
    assert!(!hs::flush(&mut t.state, &t.lock));
}

/// Upstream #10701: with OBS focused, Qt injects the press, but the hotkey
/// thread released push-to-talk within 25 ms because nothing reported the
/// key as held.
#[test]
fn focused_key_holds_push_to_talk() {
    let mut t = Thread::default();
    let ptt = t.bind(f13());

    hs::set_key(&mut t.state, &t.lock, key("OBS_KEY_F13"), true);
    t.inject(f13());
    assert!(t.bindings[ptt].pressed, "focused press");
    t.cycles();
    assert_eq!(
        t.bindings[ptt].releases, 0,
        "released while the key is held"
    );

    hs::set_key(&mut t.state, &t.lock, key("OBS_KEY_F13"), false);
    t.cycles();
    assert!(!t.bindings[ptt].pressed, "focused release");
}

/// Focused modifiers come from the keyboard's modifier state.
#[test]
fn focused_modifiers_hold_combination() {
    let mut t = Thread::default();
    let combo = t.bind(ctrl_shift_f13());

    hs::set_modifiers(
        &mut t.state,
        &t.lock,
        INTERACT_CONTROL_KEY | INTERACT_SHIFT_KEY,
    );
    hs::set_key(&mut t.state, &t.lock, key("OBS_KEY_F13"), true);
    t.inject(ctrl_shift_f13());
    assert!(t.bindings[combo].pressed, "focused ctrl+shift+F13");
    t.cycles();
    assert_eq!(t.bindings[combo].releases, 0);

    hs::set_modifiers(&mut t.state, &t.lock, INTERACT_CONTROL_KEY);
    t.cycles();
    assert!(!t.bindings[combo].pressed, "shift released");
}

/// Losing keyboard focus releases what was held, as no release event will
/// arrive for it.
#[test]
fn focus_loss_releases_keys() {
    let mut t = Thread::default();
    let ptt = t.bind(f13());

    hs::set_key(&mut t.state, &t.lock, key("OBS_KEY_F13"), true);
    t.inject(f13());
    assert!(t.bindings[ptt].pressed, "focused press");

    hs::clear_keys(&mut t.state, &t.lock);
    t.cycles();
    assert!(!t.bindings[ptt].pressed, "focus lost");
}

/// Nothing reported: the hotkey thread releases an injected press whose key
/// it cannot see.
#[test]
fn nothing_reported_releases() {
    let mut t = Thread::default();
    let ptt = t.bind(f13());

    t.inject(f13());
    assert_eq!(t.bindings[ptt].presses, 1);
    t.cycles();
    assert!(!t.bindings[ptt].pressed, "unreported key");
    for code in 0..OBS_KEY_LAST_VALUE {
        assert!(!t.pressed(i32::try_from(code).unwrap()));
    }
}

// Edges of each function.

#[test]
fn key_count_and_modifier_keys_match_the_key_list() {
    assert_eq!(OBS_KEY_LAST_VALUE, obs_hotkey::name_map::key_count());
    assert_eq!(
        hs::modifier_for_key(key("OBS_KEY_SHIFT")),
        INTERACT_SHIFT_KEY
    );
    assert_eq!(
        hs::modifier_for_key(key("OBS_KEY_CONTROL")),
        INTERACT_CONTROL_KEY
    );
    assert_eq!(hs::modifier_for_key(key("OBS_KEY_ALT")), INTERACT_ALT_KEY);
    assert_eq!(
        hs::modifier_for_key(key("OBS_KEY_META")),
        INTERACT_COMMAND_KEY
    );
    for other in [0, key("OBS_KEY_A"), key("OBS_KEY_F13"), -1, 100_000] {
        assert_eq!(hs::modifier_for_key(other), 0, "key {other}");
    }
}

/// Codes outside `OBS_KEY_NONE < key < OBS_KEY_LAST_VALUE` are ignored and
/// take no lock; valid ones lock once.
#[test]
fn out_of_range_keys_are_ignored_without_locking() {
    let lock = CountingLock::default();
    let mut s = KeyState::default();
    let last = i32::try_from(OBS_KEY_LAST_VALUE).unwrap();
    for bad in [0, -1, i32::MIN, last, last + 1, i32::MAX] {
        hs::set_key(&mut s, &lock, bad, true);
        assert!(!hs::is_pressed(&mut s, &lock, bad), "key {bad}");
        assert_eq!(lock.take(), 0, "key {bad} locked");
    }
    assert_eq!(s, KeyState::default());

    hs::set_key(&mut s, &lock, last - 1, true);
    assert!(hs::is_pressed(&mut s, &lock, last - 1));
    assert_eq!(lock.take(), 2);
}

#[test]
fn every_change_locks_once() {
    let lock = CountingLock::default();
    let mut s = KeyState::default();
    hs::set_modifiers(&mut s, &lock, INTERACT_ALT_KEY);
    assert_eq!(lock.take(), 1);
    hs::clear_keys(&mut s, &lock);
    assert_eq!(lock.take(), 1);
    hs::set_shortcut(&mut s, &lock, f13(), true);
    assert_eq!(lock.take(), 1);
    hs::set_shortcut(&mut s, &lock, ctrl_shift_f13(), false);
    assert_eq!(lock.take(), 1);
    assert!(!hs::flush(&mut s, &lock));
    assert_eq!(lock.take(), 1);
    hs::free(&mut s);
    assert_eq!(lock.take(), 0, "free takes no lock");
}

#[test]
fn focused_keys_and_modifiers_are_stored() {
    let mut s = KeyState::default();
    let a = key("OBS_KEY_A");
    hs::set_key(&mut s, &NoLock, a, true);
    assert!(s.focused_keys[usize::try_from(a).unwrap()]);
    hs::set_key(&mut s, &NoLock, a, true);
    assert!(hs::is_pressed(&mut s, &NoLock, a));
    hs::set_key(&mut s, &NoLock, a, false);
    assert!(!hs::is_pressed(&mut s, &NoLock, a));

    // Modifiers are any bits; only the four modifier keys read them.
    hs::set_modifiers(&mut s, &NoLock, u32::MAX);
    assert_eq!(s.focused_modifiers, u32::MAX);
    assert!(hs::is_pressed(&mut s, &NoLock, key("OBS_KEY_META")));
    assert!(!hs::is_pressed(&mut s, &NoLock, a));
    // A modifier key reads the modifier bits, not its own key slot.
    hs::set_modifiers(&mut s, &NoLock, 0);
    hs::set_key(&mut s, &NoLock, key("OBS_KEY_SHIFT"), true);
    assert!(!hs::is_pressed(&mut s, &NoLock, key("OBS_KEY_SHIFT")));
}

#[test]
fn clear_keys_keeps_shortcuts() {
    let mut s = KeyState::default();
    hs::set_key(&mut s, &NoLock, key("OBS_KEY_B"), true);
    hs::set_modifiers(&mut s, &NoLock, INTERACT_SHIFT_KEY);
    hs::set_shortcut(&mut s, &NoLock, f13(), true);
    hs::clear_keys(&mut s, &NoLock);
    assert!(s.focused_keys.iter().all(|k| !k));
    assert_eq!(s.focused_modifiers, 0);
    assert_eq!(s.shortcuts.len(), 1);
    assert!(hs::is_pressed(&mut s, &NoLock, key("OBS_KEY_F13")));
}

fn entry(combo: KeyCombination, observed: bool, release_pending: bool) -> Shortcut {
    Shortcut {
        combo,
        observed,
        release_pending,
    }
}

/// Activated adds one entry per combination; a repeat clears a pending
/// release. Deactivated drops an observed entry and marks an unobserved
/// one. Combinations compare key and modifiers.
#[test]
fn set_shortcut_state_machine() {
    let mut s = KeyState::default();
    let a = f13();
    let b = ctrl_shift_f13();
    hs::set_shortcut(&mut s, &NoLock, a, true);
    hs::set_shortcut(&mut s, &NoLock, a, true);
    hs::set_shortcut(&mut s, &NoLock, b, true);
    assert_eq!(
        s.shortcuts,
        [entry(a, false, false), entry(b, false, false)]
    );

    hs::set_shortcut(&mut s, &NoLock, a, false);
    assert_eq!(s.shortcuts, [entry(a, false, true), entry(b, false, false)]);
    hs::set_shortcut(&mut s, &NoLock, a, true);
    assert_eq!(s.shortcuts[0], entry(a, false, false));
    hs::set_shortcut(&mut s, &NoLock, a, false);

    // A poll observes both; Deactivated now erases at once.
    assert!(!hs::is_pressed(&mut s, &NoLock, key("OBS_KEY_A")));
    assert_eq!(s.shortcuts, [entry(a, true, true), entry(b, true, false)]);
    hs::set_shortcut(&mut s, &NoLock, b, false);
    assert_eq!(s.shortcuts, [entry(a, true, true)]);

    // Deactivated for a combination never activated does nothing.
    let other = KeyCombination {
        modifiers: INTERACT_ALT_KEY,
        key: a.key,
    };
    hs::set_shortcut(&mut s, &NoLock, other, false);
    assert_eq!(s.shortcuts, [entry(a, true, true)]);
}

/// Flush erases released entries that were observed, keeps the order of
/// the rest, and reports whether an unobserved release is left.
#[test]
fn flush_drops_observed_releases() {
    let combo = |k: &str| KeyCombination {
        modifiers: 0,
        key: key(k),
    };
    let mut s = KeyState {
        shortcuts: vec![
            entry(combo("OBS_KEY_A"), true, true),
            entry(combo("OBS_KEY_B"), false, true),
            entry(combo("OBS_KEY_C"), true, false),
            entry(combo("OBS_KEY_D"), true, true),
            entry(combo("OBS_KEY_E"), false, false),
        ],
        ..KeyState::default()
    };
    assert!(hs::flush(&mut s, &NoLock));
    assert_eq!(
        s.shortcuts,
        [
            entry(combo("OBS_KEY_B"), false, true),
            entry(combo("OBS_KEY_C"), true, false),
            entry(combo("OBS_KEY_E"), false, false),
        ]
    );
    assert!(hs::is_pressed(&mut s, &NoLock, key("OBS_KEY_B")));
    assert!(!hs::flush(&mut s, &NoLock));
    assert_eq!(s.shortcuts.len(), 2);
}

/// A shortcut presses its key and each of its modifiers, and every poll
/// marks every entry observed, whatever key it asks for.
#[test]
fn shortcuts_press_their_key_and_modifiers() {
    let mut s = KeyState::default();
    hs::set_shortcut(&mut s, &NoLock, ctrl_shift_f13(), true);
    assert!(hs::is_pressed(&mut s, &NoLock, key("OBS_KEY_F13")));
    assert!(hs::is_pressed(&mut s, &NoLock, key("OBS_KEY_CONTROL")));
    assert!(hs::is_pressed(&mut s, &NoLock, key("OBS_KEY_SHIFT")));
    assert!(!hs::is_pressed(&mut s, &NoLock, key("OBS_KEY_ALT")));
    assert!(!hs::is_pressed(&mut s, &NoLock, key("OBS_KEY_META")));
    assert!(!hs::is_pressed(&mut s, &NoLock, key("OBS_KEY_F14")));
    assert!(s.shortcuts[0].observed);

    // A modifier-only combination (OBS_KEY_NONE) presses no key slot.
    let mut s = KeyState::default();
    let alt_only = KeyCombination {
        modifiers: INTERACT_ALT_KEY,
        key: 0,
    };
    hs::set_shortcut(&mut s, &NoLock, alt_only, true);
    assert!(hs::is_pressed(&mut s, &NoLock, key("OBS_KEY_ALT")));
    assert!(!hs::is_pressed(&mut s, &NoLock, key("OBS_KEY_A")));
}

#[test]
fn free_drops_every_shortcut() {
    let mut s = KeyState::default();
    hs::set_key(&mut s, &NoLock, key("OBS_KEY_A"), true);
    hs::set_shortcut(&mut s, &NoLock, f13(), true);
    hs::free(&mut s);
    assert!(s.shortcuts.is_empty());
    // Keys are not part of what free releases.
    assert!(hs::is_pressed(&mut s, &NoLock, key("OBS_KEY_A")));
    hs::free(&mut s);
    assert!(s.shortcuts.is_empty());
}
