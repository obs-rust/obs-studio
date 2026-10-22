//! Tier 1: the safe core of `libobs/obs-hotkey-press.c.h`, the hotkey press
//! state machine.
//!
//! Two C tests characterize it, and every case of both is ported here one
//! to one, under the same name, in the same order, with the same steps and
//! the same expectations:
//! - `test/cmocka/test_hotkey_press_steps.c` (11 cases, `steps_*`) compiles
//!   the C into itself, calls the entry points one poll at a time and checks
//!   the binding's fields, its hotkey's pressed count, the callbacks and the
//!   keys queried;
//! - `test/cmocka/test_hotkey_press_state.c` (18 cases, `state_*`) holds
//!   keys with XTEST on a real libobs and waits until the hotkey thread has
//!   polled at least twice (`settle()`).
//!
//! [`World`] repeats what that C does around the core: the bindings array
//! in binding order, `obs_hotkey_load_bindings()` (remove the hotkey's
//! bindings, releasing the pressed ones, then append the new ones that
//! `obs_hotkey_combination_bindable()` accepts), `obs_hotkey_unregister()`,
//! the press and release wrappers (the pressed count; the callback, or the
//! router when rerouting), the pair trampolines, the poll of
//! `query_hotkeys()` and the walk of `obs_hotkey_inject_event()`. Its key
//! state logs every query. `settle()` is two strict polls, the least the C
//! guarantees; the state machine settles in two polls when no key changes,
//! so more would not change what is checked. The C's sentinel hotkey (F12,
//! with the held modifiers) is left out: no case binds F12, and its injects
//! in `settle_injected()` are kept, as they reach the other bindings.
//!
//! The cases after those pin the edges of the safe API.

use std::collections::BTreeSet;

use obs_hotkey::bindings::combo_bindable;
use obs_hotkey::name_map::{
    KEY_NONE, OBS_KEY_ALT, OBS_KEY_CONTROL, OBS_KEY_LAST_VALUE, OBS_KEY_META, OBS_KEY_MOUSE1,
    OBS_KEY_MOUSE3, OBS_KEY_SHIFT, key_from_name,
};
use obs_hotkey::nix_hotkey_state::{
    INTERACT_ALT_KEY, INTERACT_COMMAND_KEY, INTERACT_CONTROL_KEY, INTERACT_SHIFT_KEY,
};
use obs_hotkey::press::{
    Action, Binding, KeyCombination, MODIFIER_KEYS, inject, modifiers_match, query, query_modifiers,
};
use proptest::prelude::*;

const CTRL: u32 = INTERACT_CONTROL_KEY;
const SHIFT: u32 = INTERACT_SHIFT_KEY;
const ALT: u32 = INTERACT_ALT_KEY;
const CMD: u32 = INTERACT_COMMAND_KEY;

/// The key code named `name` (`"OBS_KEY_A"`).
fn key(name: &str) -> i32 {
    let k = key_from_name(&std::ffi::CString::new(name).unwrap());
    assert_ne!(k, KEY_NONE, "{name} is a key");
    k
}

fn a() -> i32 {
    key("OBS_KEY_A")
}
fn b() -> i32 {
    key("OBS_KEY_B")
}
fn c() -> i32 {
    key("OBS_KEY_C")
}
fn q() -> i32 {
    key("OBS_KEY_Q")
}
fn f12() -> i32 {
    key("OBS_KEY_F12")
}

fn combo(modifiers: u32, key: i32) -> KeyCombination {
    KeyCombination { modifiers, key }
}

/// One callback: the hotkey's tag (a routed one: the router's tag for the
/// id) and pressed.
type Event = (i64, bool);

#[allow(non_snake_case)]
fn P(tag: i64) -> Event {
    (tag, true)
}
#[allow(non_snake_case)]
fn R(tag: i64) -> Event {
    (tag, false)
}

const MODIFIER_QUERIES: [i32; 4] = [OBS_KEY_SHIFT, OBS_KEY_CONTROL, OBS_KEY_ALT, OBS_KEY_META];

/// What a hotkey's `func` is.
#[derive(Debug, Clone, Copy)]
enum Func {
    /// Logs its tag.
    Tagged(i64),
    /// `obs_hotkey_pair_first_func` (`which` 0) or `_second_func` (1) of
    /// pair `pair`, whose funcs log `10 + which` and return true.
    Pair { pair: usize, which: usize },
}

#[derive(Debug)]
struct Hotkey {
    func: Func,
    /// `obs_hotkey.pressed`, an `int`.
    pressed: i32,
}

#[derive(Debug, Clone, Copy)]
struct Slot {
    binding: Binding,
    /// The hotkey's id, its index in `World::hotkeys`.
    hotkey: usize,
}

/// The C around the core (see the module comment).
#[derive(Default)]
struct World {
    hotkeys: Vec<Hotkey>,
    /// `obs->hotkeys.bindings`, in binding order.
    bindings: Vec<Slot>,
    /// `pressed0` and `pressed1` of each pair.
    pairs: Vec<[bool; 2]>,
    held: BTreeSet<i32>,
    queried: Vec<i32>,
    events: Vec<Event>,
    /// `obs->hotkeys.reroute_hotkeys`.
    reroute: bool,
    /// `obs->hotkeys.router_func`: the tag it logs for a hotkey id.
    router: Option<fn(usize) -> i64>,
    /// `obs->hotkeys.thread_disable_press`.
    disable_press: bool,
}

/// test_hotkey_press_steps's router: `100 + id`.
fn steps_router(id: usize) -> i64 {
    100 + id as i64
}

/// test_hotkey_press_state's router: `-id`.
fn state_router(id: usize) -> i64 {
    -(id as i64)
}

impl World {
    fn register(&mut self, tag: i64) -> usize {
        self.hotkeys.push(Hotkey {
            func: Func::Tagged(tag),
            pressed: 0,
        });
        self.hotkeys.len() - 1
    }

    /// `obs_hotkey_pair_register_frontend`: the ids of its two hotkeys.
    fn register_pair(&mut self) -> (usize, usize) {
        self.pairs.push([false; 2]);
        let pair = self.pairs.len() - 1;
        let mut ids = [0; 2];
        for (which, id) in ids.iter_mut().enumerate() {
            self.hotkeys.push(Hotkey {
                func: Func::Pair { pair, which },
                pressed: 0,
            });
            *id = self.hotkeys.len() - 1;
        }
        (ids[0], ids[1])
    }

    /// `remove_bindings()`: a pressed binding is released, then erased.
    fn remove_bindings(&mut self, id: usize) {
        while let Some(idx) = self.bindings.iter().position(|s| s.hotkey == id) {
            if self.bindings[idx].binding.pressed {
                self.release(idx);
            }
            self.bindings.remove(idx);
        }
    }

    /// `obs_hotkey_load_bindings()`.
    fn bind_all(&mut self, id: usize, combos: &[KeyCombination]) {
        self.remove_bindings(id);
        for &key in combos {
            if combo_bindable(key) {
                self.bindings.push(Slot {
                    binding: Binding {
                        key,
                        pressed: false,
                        modifiers_match: false,
                    },
                    hotkey: id,
                });
            }
        }
    }

    fn bind(&mut self, id: usize, modifiers: u32, key: i32) {
        self.bind_all(id, &[combo(modifiers, key)]);
    }

    /// `obs_hotkey_unregister()`.
    fn unregister(&mut self, id: usize) {
        self.remove_bindings(id);
    }

    /// Hold exactly these keys.
    fn hold(&mut self, keys: &[i32]) {
        self.held = keys.iter().copied().collect();
    }

    fn key_down(&mut self, key: i32) {
        self.held.insert(key);
    }

    fn key_up(&mut self, key: i32) {
        self.held.remove(&key);
    }

    /// The hotkey's callback, or the router when rerouting
    /// (`press_released_binding()` and `release_pressed_binding()`).
    fn call(&mut self, id: usize, pressed: bool) {
        if self.reroute {
            if let Some(router) = self.router {
                self.events.push((router(id), pressed));
            }
            return;
        }
        match self.hotkeys[id].func {
            Func::Tagged(tag) => self.events.push((tag, pressed)),
            Func::Pair { pair, which } => {
                let other = 1 - which;
                if self.pairs[pair][other] {
                    return;
                }
                if self.pairs[pair][which] && !pressed {
                    self.pairs[pair][which] = false;
                } else {
                    self.events.push((10 + which as i64, pressed));
                    self.pairs[pair][which] = pressed;
                }
            }
        }
    }

    /// `obs_hotkey_binding_press()`.
    fn press(&mut self, idx: usize) {
        self.bindings[idx].binding.pressed = true;
        let id = self.bindings[idx].hotkey;
        let was = self.hotkeys[id].pressed;
        self.hotkeys[id].pressed += 1;
        if was == 0 {
            self.call(id, true);
        }
    }

    /// `obs_hotkey_binding_release()`.
    fn release(&mut self, idx: usize) {
        self.bindings[idx].binding.pressed = false;
        let id = self.bindings[idx].hotkey;
        self.hotkeys[id].pressed -= 1;
        if self.hotkeys[id].pressed == 0 {
            self.call(id, false);
        }
    }

    fn act(&mut self, idx: usize, action: Action) {
        match action {
            Action::None => {}
            Action::Press => self.press(idx),
            Action::Release => self.release(idx),
        }
    }

    /// `obs_hotkeys_query_modifiers()`.
    fn query_modifiers(&mut self) -> u32 {
        let (held, queried) = (&self.held, &mut self.queried);
        query_modifiers(|k| {
            queried.push(k);
            held.contains(&k)
        })
    }

    /// One poll of the hotkey thread: `query_hotkeys()`.
    fn poll_bindings(&mut self, no_press: bool, strict: bool) {
        let modifiers = self.query_modifiers();
        for idx in 0..self.bindings.len() {
            let mut binding = self.bindings[idx].binding;
            let (held, queried) = (&self.held, &mut self.queried);
            let action = query(&mut binding, modifiers, no_press, strict, |k| {
                queried.push(k);
                held.contains(&k)
            });
            assert_eq!(binding.pressed, self.bindings[idx].binding.pressed);
            self.bindings[idx].binding = binding;
            self.act(idx, action);
        }
    }

    fn poll_strict(&mut self) {
        self.poll_bindings(false, true);
    }

    /// The walk of `obs_hotkey_inject_event()`, with `strict` given.
    fn inject_with(&mut self, modifiers: u32, key: i32, pressed: bool, strict: bool) {
        for idx in 0..self.bindings.len() {
            let mut binding = self.bindings[idx].binding;
            let action = inject(&mut binding, combo(modifiers, key), pressed, strict);
            assert_eq!(binding.pressed, self.bindings[idx].binding.pressed);
            self.bindings[idx].binding = binding;
            self.act(idx, action);
        }
    }

    /// `obs_hotkey_inject_event()` in libobs, which is always strict.
    fn inject(&mut self, modifiers: u32, key: i32, pressed: bool) {
        self.inject_with(modifiers, key, pressed, true);
    }

    /// The held `INTERACT_*` bits, as test_hotkey_press_state tracks them.
    fn held_modifiers(&self) -> u32 {
        MODIFIER_KEYS
            .iter()
            .filter(|(k, _)| self.held.contains(k))
            .fold(0, |m, (_, bit)| m | bit)
    }

    /// test_hotkey_press_state's `settle()`: two whole polls.
    fn settle(&mut self) {
        for _ in 0..2 {
            self.poll_bindings(self.disable_press, true);
        }
    }

    /// `settle_injected()`: the sentinel press is injected (held modifiers
    /// and F12), and the next poll releases it; twice.
    fn settle_injected(&mut self) {
        for _ in 0..2 {
            let modifiers = self.held_modifiers();
            self.inject(modifiers, f12(), true);
            self.poll_bindings(self.disable_press, true);
        }
    }

    #[track_caller]
    fn expect(&mut self, expected: &[Event]) {
        let events = std::mem::take(&mut self.events);
        assert_eq!(events, expected, "callbacks");
    }

    #[track_caller]
    fn expect_none(&mut self) {
        self.expect(&[]);
    }

    #[track_caller]
    fn expect_queried(&mut self, expected: &[i32]) {
        let queried = std::mem::take(&mut self.queried);
        assert_eq!(queried, expected, "keys queried");
    }

    #[track_caller]
    fn expect_modifier_queries_then(&mut self, then: &[i32]) {
        let mut expected = MODIFIER_QUERIES.to_vec();
        expected.extend_from_slice(then);
        self.expect_queried(&expected);
    }

    #[track_caller]
    fn expect_binding(&self, idx: usize, pressed: bool, modifiers_match: bool) {
        let b = self.bindings[idx].binding;
        assert_eq!(
            (b.pressed, b.modifiers_match),
            (pressed, modifiers_match),
            "binding {idx}: (pressed, modifiers_match)"
        );
    }

    fn pressed_count(&self, id: usize) -> i32 {
        self.hotkeys[id].pressed
    }
}

fn steps_world() -> World {
    World {
        router: None,
        ..World::default()
    }
}

// ---------------------------------------------------------------------------
// test_hotkey_press_steps.c

/// `query_modifiers`: Shift, Control, Alt and Meta, once each and in that
/// order, and Meta maps to the command bit.
#[test]
fn steps_query_modifiers() {
    let mut w = steps_world();
    assert_eq!(w.query_modifiers(), 0);
    w.expect_queried(&MODIFIER_QUERIES);

    w.hold(&[OBS_KEY_SHIFT, OBS_KEY_ALT, a()]);
    assert_eq!(w.query_modifiers(), SHIFT | ALT);
    w.expect_queried(&MODIFIER_QUERIES);

    w.hold(&[OBS_KEY_CONTROL, OBS_KEY_META]);
    assert_eq!(w.query_modifiers(), CTRL | CMD);
    w.expect_queried(&MODIFIER_QUERIES);
}

/// `latch_steps`: strict Ctrl+A, poll by poll.
#[test]
fn steps_latch_steps() {
    let mut w = steps_world();
    let h = w.register(1);
    w.bind(h, CTRL, a());
    w.expect_binding(0, false, false);

    w.hold(&[OBS_KEY_CONTROL, a()]);
    w.poll_strict();
    w.expect_binding(0, false, true);
    w.expect_none();
    w.poll_strict();
    w.expect_binding(0, true, true);
    w.expect(&[P(1)]);
    w.poll_strict();
    w.expect_binding(0, true, true);
    w.expect_none();

    // Ctrl released: released, latch false.
    w.hold(&[a()]);
    w.poll_strict();
    w.expect_binding(0, false, false);
    w.expect(&[R(1)]);

    // Ctrl again: one poll to set the latch, the next presses.
    w.hold(&[OBS_KEY_CONTROL, a()]);
    w.poll_strict();
    w.expect_binding(0, false, true);
    w.expect_none();
    w.poll_strict();
    w.expect_binding(0, true, true);
    w.expect(&[P(1)]);

    // A released, Ctrl still held: released, latch stays true, so A again
    // presses on the next poll.
    w.hold(&[OBS_KEY_CONTROL]);
    w.poll_strict();
    w.expect_binding(0, false, true);
    w.expect(&[R(1)]);
    w.hold(&[OBS_KEY_CONTROL, a()]);
    w.poll_strict();
    w.expect_binding(0, true, true);
    w.expect(&[P(1)]);
}

/// `new_binding_second_poll`: a strict binding without modifiers, held from
/// the start, is pressed on the second poll.
#[test]
fn steps_new_binding_second_poll() {
    let mut w = steps_world();
    let h = w.register(1);
    w.bind(h, 0, a());
    w.hold(&[a()]);

    w.poll_strict();
    w.expect_binding(0, false, true);
    w.expect_none();
    w.poll_strict();
    w.expect_binding(0, true, true);
    w.expect(&[P(1)]);
}

/// `key_state_is_lazy`: the key is queried only after the modifiers and the
/// latch match, and before `no_press` is looked at; never for a
/// modifier-only or an empty binding, and never for an inject.
#[test]
fn steps_key_state_is_lazy() {
    let mut w = steps_world();
    let h = w.register(1);
    w.bind(h, CTRL, a());
    w.queried.clear();

    // Modifiers differ: no key query.
    w.poll_strict();
    w.expect_queried(&MODIFIER_QUERIES);

    // Modifiers match, latch false: no key query.
    w.hold(&[OBS_KEY_CONTROL]);
    w.poll_strict();
    w.expect_queried(&MODIFIER_QUERIES);

    // Latch true: one query of A per poll, pressed or not, with presses on
    // or off.
    w.poll_strict();
    w.expect_modifier_queries_then(&[a()]);
    w.hold(&[OBS_KEY_CONTROL, a()]);
    w.poll_bindings(true, true);
    w.expect_modifier_queries_then(&[a()]);
    w.poll_strict();
    w.expect_modifier_queries_then(&[a()]);
    w.expect(&[P(1)]);
    w.poll_strict();
    w.expect_modifier_queries_then(&[a()]);
    w.expect_none();

    // A modifier-only and an empty binding: no key query, in either branch.
    let m = w.register(2);
    let e = w.register(3);
    w.bind(h, 0, KEY_NONE);
    w.expect(&[R(1)]);
    w.bind(m, CTRL, KEY_NONE);
    w.bind(e, 0, KEY_NONE);
    w.queried.clear();
    for _ in 0..3 {
        w.poll_strict();
        w.expect_queried(&MODIFIER_QUERIES);
        w.poll_bindings(false, false);
        w.expect_queried(&MODIFIER_QUERIES);
    }
    w.expect(&[P(2)]);

    // Injecting queries nothing.
    w.inject_with(CTRL, a(), true, true);
    w.inject_with(0, a(), false, false);
    w.expect_queried(&[]);
    w.expect(&[P(1), P(3)]);
}

/// `modifier_only_steps`: pressed on the first poll with exactly its
/// modifiers, released when they change; a poll never sets its latch.
#[test]
fn steps_modifier_only_steps() {
    let mut w = steps_world();
    let h = w.register(1);
    w.bind(h, CTRL | SHIFT, KEY_NONE);

    w.hold(&[OBS_KEY_CONTROL, OBS_KEY_SHIFT]);
    w.poll_strict();
    w.expect_binding(0, true, false);
    w.expect(&[P(1)]);

    w.hold(&[OBS_KEY_CONTROL, OBS_KEY_SHIFT, OBS_KEY_ALT]);
    w.poll_strict();
    w.expect_binding(0, false, false);
    w.expect(&[R(1)]);

    w.hold(&[OBS_KEY_CONTROL, OBS_KEY_SHIFT]);
    w.poll_strict();
    w.expect_binding(0, true, false);
    w.expect(&[P(1)]);
}

/// `empty_binding`: never pressed by a poll; injected events press it.
/// Characterized, not endorsed.
#[test]
fn steps_empty_binding() {
    let mut w = steps_world();
    let h = w.register(1);
    w.bind(h, 0, KEY_NONE);

    w.hold(&[a()]);
    w.poll_strict();
    w.poll_bindings(false, false);
    w.expect_binding(0, false, true);
    w.expect_none();

    w.inject_with(SHIFT, a(), true, true);
    w.expect_none();
    w.inject_with(0, b(), false, true);
    w.expect_binding(0, true, true);
    w.expect(&[P(1)]);
    w.poll_strict();
    w.expect_binding(0, false, true);
    w.expect(&[R(1)]);

    w.inject_with(CTRL | ALT, KEY_NONE, false, false);
    w.expect(&[P(1)]);
    w.poll_bindings(false, false);
    w.expect(&[R(1)]);
}

/// `non_strict`: a binding's modifiers must all be held, others may be too;
/// a binding without modifiers has its latch set on every poll.
#[test]
fn steps_non_strict() {
    let mut w = steps_world();
    let ha = w.register(1);
    let hb = w.register(2);
    let hm = w.register(3);
    w.bind(ha, 0, a());
    w.bind(hb, CTRL, b());
    w.bind(hm, ALT, KEY_NONE);

    w.hold(&[OBS_KEY_CONTROL, OBS_KEY_SHIFT, a()]);
    w.poll_bindings(false, false);
    w.expect_binding(0, true, true);
    w.expect_binding(1, false, true);
    w.expect_binding(2, false, false);
    w.expect(&[P(1)]);

    w.hold(&[OBS_KEY_CONTROL, OBS_KEY_SHIFT, OBS_KEY_ALT, a(), b()]);
    w.poll_bindings(false, false);
    w.expect_binding(1, true, true);
    w.expect_binding(2, true, false);
    w.expect(&[P(2), P(3)]);

    w.hold(&[OBS_KEY_SHIFT, OBS_KEY_ALT, b()]);
    w.poll_bindings(false, false);
    w.expect_binding(0, false, true);
    w.expect_binding(1, false, false);
    w.expect_binding(2, true, false);
    w.expect(&[R(1), R(2)]);

    // Injected, not strict: a superset of the modifiers matches.
    w.inject_with(CTRL | SHIFT, b(), true, false);
    w.expect_binding(1, true, true);
    w.expect(&[P(2)]);
}

/// `no_press_steps`: nothing is pressed and the latch is not touched while
/// the checks pass; a pressed binding stays pressed while held and is
/// released when its key is.
#[test]
fn steps_no_press_steps() {
    let mut w = steps_world();
    let h = w.register(1);
    w.bind(h, 0, a());

    w.hold(&[a()]);
    w.poll_bindings(true, true);
    w.poll_bindings(true, true);
    w.expect_binding(0, false, true);
    w.expect_none();

    w.poll_strict();
    w.expect_binding(0, true, true);
    w.expect(&[P(1)]);
    w.poll_bindings(true, true);
    w.expect_binding(0, true, true);
    w.expect_none();

    w.hold(&[]);
    w.poll_bindings(true, true);
    w.expect_binding(0, false, true);
    w.expect(&[R(1)]);
}

/// `inject_steps`: an inject that presses sets the latch even where the
/// binding is already pressed; one that does not press leaves the binding
/// alone; `thread_disable_press` does not apply to it.
#[test]
fn steps_inject_steps() {
    let mut w = steps_world();
    let h = w.register(1);
    w.bind(h, CTRL, a());

    w.inject_with(CTRL, a(), false, true);
    w.inject_with(CTRL, b(), true, true);
    w.inject_with(0, a(), true, true);
    w.expect_binding(0, false, false);
    w.expect_none();

    // thread_disable_press is not an input of an inject.
    w.disable_press = true;
    w.inject_with(CTRL, a(), true, true);
    w.expect_binding(0, true, true);
    w.expect(&[P(1)]);

    // Held: the poll keeps it pressed; an injected release changes nothing.
    w.hold(&[OBS_KEY_CONTROL, a()]);
    w.poll_strict();
    w.inject_with(CTRL, a(), false, true);
    w.expect_binding(0, true, true);
    w.expect_none();

    // Ctrl released: the poll releases and clears the latch; an injected
    // press sets it again.
    w.hold(&[a()]);
    w.poll_strict();
    w.expect_binding(0, false, false);
    w.expect(&[R(1)]);
    w.inject_with(CTRL, a(), true, true);
    w.expect_binding(0, true, true);
    w.expect(&[P(1)]);
    w.inject_with(CTRL, a(), true, true);
    w.expect_none();
}

/// `pressed_count_and_routing`: the callback runs on the first press and
/// the last release; rerouted, the router gets them; with no router
/// nothing is called, but the count still moves.
#[test]
fn steps_pressed_count_and_routing() {
    let mut w = steps_world();
    let h = w.register(1);
    w.bind_all(h, &[combo(0, a()), combo(0, OBS_KEY_MOUSE3)]);
    assert_eq!(w.bindings[0].hotkey, w.bindings[1].hotkey);

    w.inject_with(0, a(), true, true);
    assert_eq!(w.pressed_count(h), 1);
    w.expect(&[P(1)]);
    w.inject_with(0, OBS_KEY_MOUSE3, true, true);
    assert_eq!(w.pressed_count(h), 2);
    w.expect_none();

    w.hold(&[OBS_KEY_MOUSE3]);
    w.poll_strict();
    assert_eq!(w.pressed_count(h), 1);
    w.expect_binding(0, false, true);
    w.expect_binding(1, true, true);
    w.expect_none();
    w.hold(&[]);
    w.poll_strict();
    assert_eq!(w.pressed_count(h), 0);
    w.expect(&[R(1)]);

    w.reroute = true;
    w.router = Some(steps_router);
    w.inject_with(0, a(), true, true);
    w.poll_strict();
    w.expect(&[P(steps_router(h)), R(steps_router(h))]);

    w.router = None;
    w.inject_with(0, a(), true, true);
    assert_eq!(w.pressed_count(h), 1);
    w.expect_none();
    w.router = Some(steps_router);
    w.poll_strict();
    assert_eq!(w.pressed_count(h), 0);
    w.expect(&[R(steps_router(h))]);
}

/// `binding_fields`: what the state machine reads and writes is the
/// binding's own state. Here the binding is the core's [`Binding`] itself:
/// a latch set on it is the one the next poll reads, and a release or an
/// inject shows on it.
#[test]
fn steps_binding_fields() {
    let mut w = steps_world();
    let h = w.register(1);
    let combos = [combo(CTRL, a()), combo(0, OBS_KEY_MOUSE3)];
    w.bind_all(h, &combos);

    for (i, c) in combos.iter().enumerate() {
        let binding = w.bindings[i].binding;
        assert_eq!(binding.key, *c);
        assert!(!binding.pressed);
        assert!(!binding.modifiers_match);
    }

    // A latch set from outside is the one a poll reads: Ctrl+A, held, is
    // pressed on the first poll instead of the second.
    w.bindings[0].binding.modifiers_match = true;
    w.hold(&[OBS_KEY_CONTROL, a()]);
    w.poll_strict();
    assert!(w.bindings[0].binding.pressed);
    assert!(w.bindings[0].binding.modifiers_match);
    w.expect(&[P(1)]);

    // A released, Ctrl held: released, latch kept.
    w.hold(&[OBS_KEY_CONTROL]);
    w.poll_strict();
    assert!(!w.bindings[0].binding.pressed);
    assert!(w.bindings[0].binding.modifiers_match);
    w.expect(&[R(1)]);

    // An inject presses the second binding.
    w.inject_with(0, OBS_KEY_MOUSE3, true, true);
    assert!(w.bindings[1].binding.pressed);
    assert!(w.bindings[1].binding.modifiers_match);
    w.expect(&[P(1)]);
}

// ---------------------------------------------------------------------------
// test_hotkey_press_state.c (libobs: strict modifiers, settle() polls)

fn state_world() -> World {
    World::default()
}

/// `key_press_and_release`.
#[test]
fn state_key_press_and_release() {
    let mut w = state_world();
    let h = w.register(1);
    w.bind(h, 0, a());
    w.settle();

    w.key_down(a());
    w.settle();
    w.expect(&[P(1)]);

    w.key_up(a());
    w.settle();
    w.expect(&[R(1)]);
    w.unregister(h);
}

/// `modifier_combination`: Ctrl+A.
#[test]
fn state_modifier_combination() {
    let mut w = state_world();
    let h = w.register(1);
    w.bind(h, CTRL, a());
    w.settle();

    w.key_down(a());
    w.settle();
    w.expect_none();
    w.key_up(a());

    w.key_down(OBS_KEY_CONTROL);
    w.settle();
    w.expect_none();
    w.key_down(a());
    w.settle();
    w.expect(&[P(1)]);

    w.key_up(OBS_KEY_CONTROL);
    w.settle();
    w.expect(&[R(1)]);

    w.key_down(OBS_KEY_CONTROL);
    w.settle();
    w.expect(&[P(1)]);

    w.key_up(a());
    w.settle();
    w.expect(&[R(1)]);
    w.key_up(OBS_KEY_CONTROL);
    w.settle();
    w.expect_none();
    w.unregister(h);
}

/// `strict_modifiers`.
#[test]
fn state_strict_modifiers() {
    let mut w = state_world();
    let ha = w.register(1);
    let hb = w.register(2);
    w.bind(ha, CTRL, a());
    w.bind(hb, 0, b());
    w.settle();

    w.key_down(OBS_KEY_CONTROL);
    w.key_down(OBS_KEY_SHIFT);
    w.key_down(a());
    w.settle();
    w.expect_none();
    w.key_up(a());
    w.key_up(OBS_KEY_SHIFT);

    w.key_down(b());
    w.settle();
    w.expect_none();
    w.key_up(OBS_KEY_CONTROL);
    w.settle();
    w.expect(&[P(2)]);

    w.key_down(OBS_KEY_ALT);
    w.settle();
    w.expect(&[R(2)]);
    w.key_up(OBS_KEY_ALT);
    w.settle();
    w.expect(&[P(2)]);
    w.key_up(b());
    w.settle();
    w.expect(&[R(2)]);
    w.unregister(ha);
    w.unregister(hb);
}

/// `key_before_modifier`: characterized, not endorsed.
#[test]
fn state_key_before_modifier() {
    let mut w = state_world();
    let h = w.register(1);
    w.bind(h, CTRL, a());
    w.settle();

    w.key_down(a());
    w.settle();
    w.expect_none();
    w.key_down(OBS_KEY_CONTROL);
    w.settle();
    w.expect(&[P(1)]);

    w.key_up(OBS_KEY_CONTROL);
    w.key_up(a());
    w.settle();
    w.expect(&[R(1)]);
    w.unregister(h);
}

/// `modifier_only_binding`.
#[test]
fn state_modifier_only_binding() {
    let mut w = state_world();
    let h = w.register(1);
    w.bind(h, CTRL, KEY_NONE);
    w.settle();

    w.key_down(a());
    w.settle();
    w.expect_none();
    w.key_down(OBS_KEY_CONTROL);
    w.settle();
    w.expect(&[P(1)]);

    w.key_up(a());
    w.settle();
    w.expect_none();

    w.key_down(OBS_KEY_SHIFT);
    w.settle();
    w.expect(&[R(1)]);
    w.key_up(OBS_KEY_SHIFT);
    w.settle();
    w.expect(&[P(1)]);

    w.key_up(OBS_KEY_CONTROL);
    w.settle();
    w.expect(&[R(1)]);
    w.unregister(h);
}

/// `two_bindings_one_hotkey`.
#[test]
fn state_two_bindings_one_hotkey() {
    let mut w = state_world();
    let h = w.register(1);
    w.bind_all(h, &[combo(0, a()), combo(0, b())]);
    w.settle();

    w.key_down(a());
    w.settle();
    w.expect(&[P(1)]);
    w.key_down(b());
    w.settle();
    w.expect_none();
    w.key_up(a());
    w.settle();
    w.expect_none();
    w.key_down(a());
    w.settle();
    w.expect_none();
    w.key_up(b());
    w.settle();
    w.expect_none();
    w.key_up(a());
    w.settle();
    w.expect(&[R(1)]);
    w.unregister(h);
}

/// `duplicate_bindings`.
#[test]
fn state_duplicate_bindings() {
    let mut w = state_world();
    let h = w.register(1);
    w.bind_all(h, &[combo(CTRL, a()), combo(CTRL, a())]);
    w.settle();

    w.key_down(OBS_KEY_CONTROL);
    w.key_down(a());
    w.settle();
    w.expect(&[P(1)]);
    w.key_up(a());
    w.settle();
    w.expect(&[R(1)]);
    w.unregister(h);
}

/// `two_hotkeys_one_key`: in binding order.
#[test]
fn state_two_hotkeys_one_key() {
    let mut w = state_world();
    let ha = w.register(1);
    let hb = w.register(2);
    w.bind(hb, 0, a());
    w.bind(ha, 0, a());
    w.settle();

    w.key_down(a());
    w.settle();
    w.expect(&[P(2), P(1)]);
    w.key_up(a());
    w.settle();
    w.expect(&[R(2), R(1)]);
    w.unregister(ha);
    w.unregister(hb);
}

/// `rebind_while_pressed`.
#[test]
fn state_rebind_while_pressed() {
    let mut w = state_world();
    let h = w.register(1);
    w.bind(h, 0, a());
    w.settle();

    w.key_down(a());
    w.settle();
    w.expect(&[P(1)]);
    w.bind(h, 0, b());
    w.expect(&[R(1)]);
    w.settle();
    w.expect_none();
    w.key_down(b());
    w.settle();
    w.expect(&[P(1)]);
    w.unregister(h);
    w.expect(&[R(1)]);
}

/// `rerouted`.
#[test]
fn state_rerouted() {
    let mut w = state_world();
    let h = w.register(1);
    w.bind(h, 0, a());
    w.router = Some(state_router);
    w.reroute = true;
    w.settle();

    w.key_down(a());
    w.settle();
    w.expect(&[P(state_router(h))]);
    w.key_up(a());
    w.settle();
    w.expect(&[R(state_router(h))]);
    w.unregister(h);
}

/// `rerouted_without_router`: characterized, not endorsed.
#[test]
fn state_rerouted_without_router() {
    let mut w = state_world();
    let h = w.register(1);
    w.bind(h, 0, a());
    w.router = None;
    w.reroute = true;

    // inject_then_route, under the hotkey mutex.
    w.inject(0, a(), true);
    w.router = Some(state_router);
    w.expect_none();
    w.settle();
    w.expect(&[R(state_router(h))]);
    w.unregister(h);
}

/// `background_press_disabled`.
#[test]
fn state_background_press_disabled() {
    let mut w = state_world();
    let h = w.register(1);
    w.bind(h, 0, a());
    w.settle();

    w.disable_press = true;
    w.key_down(a());
    w.settle_injected();
    w.expect_none();
    w.key_up(a());
    w.settle_injected();
    w.expect_none();

    w.disable_press = false;
    w.key_down(a());
    w.settle();
    w.expect(&[P(1)]);
    w.disable_press = true;
    w.settle_injected();
    w.expect_none();
    w.key_up(a());
    w.settle_injected();
    w.expect(&[R(1)]);

    w.key_down(a());
    w.settle_injected();
    w.expect_none();
    w.disable_press = false;
    w.settle();
    w.expect(&[P(1)]);
    w.key_up(a());
    w.settle();
    w.expect(&[R(1)]);
    w.unregister(h);
}

/// `pair`.
#[test]
fn state_pair() {
    let mut w = state_world();
    let (h0, h1) = w.register_pair();
    w.bind(h0, 0, a());
    w.bind(h1, 0, b());
    w.settle();

    w.key_down(a());
    w.settle();
    w.expect(&[P(10)]);
    w.key_down(b());
    w.settle();
    w.expect_none();
    w.key_up(b());
    w.settle();
    w.expect_none();
    w.key_up(a());
    w.settle();
    w.expect_none();

    w.key_down(b());
    w.settle();
    w.expect(&[P(11)]);
    w.key_up(b());
    w.settle();
    w.expect_none();
    w.unregister(h0);
    w.unregister(h1);
}

/// `inject_press_then_poll_release`.
#[test]
fn state_inject_press_then_poll_release() {
    let mut w = state_world();
    let ha = w.register(1);
    let hm = w.register(2);
    w.bind(ha, CTRL, a());
    w.bind(hm, 0, OBS_KEY_MOUSE3);
    w.settle();

    // inject_twice, under the hotkey mutex.
    w.inject(CTRL, a(), true);
    w.inject(CTRL, a(), true);
    w.inject(0, OBS_KEY_MOUSE3, true);
    w.settle();
    w.expect(&[P(1), P(2), R(1), R(2)]);
    w.unregister(ha);
    w.unregister(hm);
}

/// `inject_release_ignored`.
#[test]
fn state_inject_release_ignored() {
    let mut w = state_world();
    let h = w.register(1);
    w.bind(h, 0, a());
    w.settle();

    w.key_down(a());
    w.settle();
    w.expect(&[P(1)]);
    w.inject(0, a(), false);
    w.settle();
    w.expect_none();
    w.key_up(a());
    w.settle();
    w.expect(&[R(1)]);

    w.inject(0, a(), false);
    w.inject(0, a(), true);
    w.inject(0, a(), false);
    w.settle();
    w.expect(&[P(1), R(1)]);
    w.unregister(h);
}

/// `inject_strict_and_background`.
#[test]
fn state_inject_strict_and_background() {
    let mut w = state_world();
    let ha = w.register(1);
    let hb = w.register(2);
    w.bind(ha, 0, a());
    w.bind(hb, CTRL, b());
    w.settle();

    w.inject(CTRL, a(), true);
    w.inject(0, b(), true);
    w.inject(CTRL | SHIFT, b(), true);
    w.inject(CTRL, c(), true);
    w.settle();
    w.expect_none();

    w.disable_press = true;
    w.inject(0, a(), true);
    w.settle_injected();
    w.expect(&[P(1), R(1)]);
    w.unregister(ha);
    w.unregister(hb);
}

/// `inject_modifier_only`: characterized, not endorsed.
#[test]
fn state_inject_modifier_only() {
    let mut w = state_world();
    let h = w.register(1);
    w.bind(h, CTRL, KEY_NONE);
    w.settle();

    w.inject(0, b(), true);
    w.inject(SHIFT, KEY_NONE, true);
    w.settle();
    w.expect_none();

    w.inject(CTRL, b(), false);
    w.settle();
    w.expect(&[P(1), R(1)]);
    w.unregister(h);
}

/// `inject_empty_binding`: characterized, not endorsed.
#[test]
fn state_inject_empty_binding() {
    let mut w = state_world();
    let h = w.register(1);
    w.bind(h, 0, KEY_NONE);
    w.settle();

    w.key_down(a());
    w.settle();
    w.expect_none();
    w.key_up(a());

    w.inject(0, q(), false);
    w.settle();
    w.expect(&[P(1), R(1)]);
    w.inject(CTRL, q(), true);
    w.settle();
    w.expect_none();
    w.unregister(h);
}

// ---------------------------------------------------------------------------
// The safe API

fn binding(modifiers: u32, key: i32, pressed: bool, modifiers_match: bool) -> Binding {
    Binding {
        key: combo(modifiers, key),
        pressed,
        modifiers_match,
    }
}

/// The modifier table: the C's order and bits, Meta as the command bit.
#[test]
fn modifier_keys_order_and_bits() {
    assert_eq!(
        MODIFIER_KEYS,
        [
            (OBS_KEY_SHIFT, SHIFT),
            (OBS_KEY_CONTROL, CTRL),
            (OBS_KEY_ALT, ALT),
            (OBS_KEY_META, CMD),
        ]
    );
}

/// Each modifier alone, and all of them; every one is queried even when
/// the earlier ones are held (no short circuit).
#[test]
fn query_modifiers_each_bit() {
    for (k, bit) in MODIFIER_KEYS {
        let mut log = Vec::new();
        let m = query_modifiers(|q| {
            log.push(q);
            q == k
        });
        assert_eq!(m, bit);
        assert_eq!(log, MODIFIER_QUERIES);
    }
    let mut log = Vec::new();
    assert_eq!(
        query_modifiers(|q| {
            log.push(q);
            true
        }),
        SHIFT | CTRL | ALT | CMD
    );
    assert_eq!(log, MODIFIER_QUERIES);
}

/// Strict compares every bit, also bits other than the four modifiers;
/// not strict is a subset test over every bit.
#[test]
fn modifiers_match_every_bit() {
    let caps = 1u32; // INTERACT_CAPS_KEY
    assert!(modifiers_match(0, 0, true));
    assert!(!modifiers_match(0, CTRL, true));
    assert!(!modifiers_match(CTRL, 0, true));
    assert!(modifiers_match(CTRL | ALT, ALT | CTRL, true));
    assert!(!modifiers_match(CTRL, CTRL | caps, true));
    assert!(modifiers_match(caps, caps, true));
    assert!(modifiers_match(0, u32::MAX, false));
    assert!(modifiers_match(CTRL, CTRL | SHIFT, false));
    assert!(!modifiers_match(CTRL | SHIFT, CTRL, false));
    assert!(modifiers_match(u32::MAX, u32::MAX, true));
    assert!(modifiers_match(u32::MAX, u32::MAX, false));
    assert!(!modifiers_match(u32::MAX, u32::MAX - 1, false));
}

/// The key state gets the binding's key as it is, whatever it is: out of
/// range, negative, a mouse button.
#[test]
fn query_passes_any_key_through() {
    for k in [
        -1,
        i32::MIN,
        i32::MAX,
        OBS_KEY_LAST_VALUE as i32,
        OBS_KEY_MOUSE1,
        OBS_KEY_MOUSE3,
    ] {
        let mut bi = binding(0, k, false, true);
        let mut seen = None;
        let action = query(&mut bi, 0, false, true, |q| {
            seen = Some(q);
            true
        });
        assert_eq!(seen, Some(k));
        assert_eq!(action, Action::Press);
        let action = inject(&mut binding(0, k, false, false), combo(0, k), true, true);
        assert_eq!(action, Action::Press);
    }
}

/// The reset path: a pressed binding is released, its latch set to whether
/// the modifiers match now; an unpressed one is left unpressed.
#[test]
fn query_reset_sets_latch_to_match() {
    // Modifiers do not match: latch false.
    let mut bi = binding(CTRL, a(), true, true);
    assert_eq!(
        query(&mut bi, 0, false, true, |_| unreachable!()),
        Action::Release
    );
    assert!(!bi.modifiers_match);
    // Modifiers match, key up: latch true.
    let mut bi = binding(CTRL, a(), true, true);
    assert_eq!(
        query(&mut bi, CTRL, false, true, |_| false),
        Action::Release
    );
    assert!(bi.modifiers_match);
    // Not pressed: nothing to release.
    let mut bi = binding(CTRL, a(), false, false);
    assert_eq!(
        query(&mut bi, CTRL, true, true, |_| unreachable!()),
        Action::None
    );
    assert!(bi.modifiers_match);
}

/// `no_press` is looked at after the key: with the key up, a pressed
/// binding is released even with presses off.
#[test]
fn no_press_blocks_presses_only() {
    let mut bi = binding(0, a(), false, true);
    assert_eq!(query(&mut bi, 0, true, true, |_| true), Action::None);
    assert!(bi.modifiers_match);
    let mut bi = binding(0, a(), true, true);
    assert_eq!(query(&mut bi, 0, true, true, |_| true), Action::None);
    assert_eq!(query(&mut bi, 0, true, true, |_| false), Action::Release);
}

/// A pressed binding whose checks still pass is left alone.
#[test]
fn query_pressed_and_held_does_nothing() {
    let mut bi = binding(SHIFT, a(), true, true);
    assert_eq!(query(&mut bi, SHIFT, false, true, |_| true), Action::None);
    assert_eq!(bi, binding(SHIFT, a(), true, true));
}

/// Not strict, a binding without modifiers gets its latch set before the
/// checks, so its first poll can press; one with modifiers does not.
#[test]
fn non_strict_forces_latch_without_modifiers() {
    let mut bi = binding(0, a(), false, false);
    assert_eq!(query(&mut bi, CTRL, false, false, |_| true), Action::Press);
    assert!(bi.modifiers_match);
    // Key up: reset, latch = match (true, the empty set is a subset).
    let mut bi = binding(0, a(), false, false);
    assert_eq!(query(&mut bi, CTRL, false, false, |_| false), Action::None);
    assert!(bi.modifiers_match);
    let mut bi = binding(CTRL, a(), false, false);
    assert_eq!(
        query(&mut bi, CTRL, false, false, |_| unreachable!()),
        Action::None
    );
    assert!(bi.modifiers_match);
    // Strict, without modifiers: no forcing.
    let mut bi = binding(0, a(), false, false);
    assert_eq!(
        query(&mut bi, 0, false, true, |_| unreachable!()),
        Action::None
    );
    assert!(bi.modifiers_match);
}

/// A modifier-only binding ignores its latch on a poll and never queries;
/// an empty binding is never pressed by a poll and is released by one.
#[test]
fn modifier_only_and_empty_bindings_on_a_poll() {
    let mut bi = binding(CTRL, KEY_NONE, false, false);
    assert_eq!(
        query(&mut bi, CTRL, false, true, |_| unreachable!()),
        Action::Press
    );
    assert!(!bi.modifiers_match);
    let mut bi = binding(CTRL, KEY_NONE, false, true);
    assert_eq!(
        query(&mut bi, CTRL, true, true, |_| unreachable!()),
        Action::None
    );
    assert!(bi.modifiers_match, "untouched on the press path");

    for strict in [true, false] {
        let mut bi = binding(0, KEY_NONE, true, false);
        assert_eq!(
            query(&mut bi, 0, false, strict, |_| unreachable!()),
            Action::Release
        );
        assert!(bi.modifiers_match);
        let mut bi = binding(0, KEY_NONE, false, false);
        assert_eq!(
            query(&mut bi, SHIFT, false, strict, |_| unreachable!()),
            Action::None
        );
        assert_eq!(bi.modifiers_match, !strict);
    }
}

/// An inject never releases, never needs the key state, and sets the
/// latch whenever it would press, pressed or not.
#[test]
fn inject_rules() {
    // Already pressed: latch set, no action.
    let mut bi = binding(CTRL, a(), true, false);
    assert_eq!(inject(&mut bi, combo(CTRL, a()), true, true), Action::None);
    assert!(bi.modifiers_match);
    // A release event of the binding's key: nothing.
    let mut bi = binding(CTRL, a(), true, false);
    assert_eq!(inject(&mut bi, combo(CTRL, a()), false, true), Action::None);
    assert!(!bi.modifiers_match);
    // Another key: nothing.
    let mut bi = binding(CTRL, a(), false, false);
    assert_eq!(inject(&mut bi, combo(CTRL, b()), true, true), Action::None);
    assert_eq!(bi, binding(CTRL, a(), false, false));
    // Modifiers do not match: nothing, latch untouched.
    let mut bi = binding(CTRL, a(), false, true);
    assert_eq!(inject(&mut bi, combo(0, a()), true, true), Action::None);
    assert!(bi.modifiers_match);
    // No key: any event with the modifiers, press or release.
    for pressed in [true, false] {
        for k in [a(), KEY_NONE, -5] {
            let mut bi = binding(ALT, KEY_NONE, false, false);
            assert_eq!(inject(&mut bi, combo(ALT, k), pressed, true), Action::Press);
            assert!(bi.modifiers_match);
        }
    }
    // An event with OBS_KEY_NONE presses a key binding? No: the key must be
    // the binding's.
    let mut bi = binding(0, a(), false, false);
    assert_eq!(
        inject(&mut bi, combo(0, KEY_NONE), true, true),
        Action::None
    );
}

fn any_key() -> impl Strategy<Value = i32> {
    prop_oneof![
        Just(KEY_NONE),
        Just(OBS_KEY_SHIFT),
        Just(OBS_KEY_CONTROL),
        Just(OBS_KEY_MOUSE1),
        Just(OBS_KEY_MOUSE3),
        (1..OBS_KEY_LAST_VALUE as i32),
        any::<i32>(),
    ]
}

fn any_modifiers() -> impl Strategy<Value = u32> {
    prop_oneof![
        4 => (0u32..16).prop_map(|m| {
            [SHIFT, CTRL, ALT, CMD]
                .iter()
                .enumerate()
                .filter(|(i, _)| m & (1 << i) != 0)
                .fold(0, |acc, (_, b)| acc | b)
        }),
        1 => any::<u32>(),
    ]
}

fn any_binding() -> impl Strategy<Value = Binding> {
    (any_modifiers(), any_key(), any::<bool>(), any::<bool>()).prop_map(|(m, k, p, l)| Binding {
        key: combo(m, k),
        pressed: p,
        modifiers_match: l,
    })
}

proptest! {
    /// The invariants of one poll: the pressed flag is never changed; a
    /// press only for an unpressed binding with presses on, a release only
    /// for a pressed one; the key state at most once (FnOnce), with the
    /// binding's key, never for a modifier-only or empty binding; an empty
    /// binding is never pressed.
    #[test]
    fn query_invariants(
        bi in any_binding(),
        modifiers in any_modifiers(),
        no_press in any::<bool>(),
        strict in any::<bool>(),
        held in any::<bool>(),
    ) {
        let mut after = bi;
        let mut seen = None;
        let action = query(&mut after, modifiers, no_press, strict, |k| {
            seen = Some(k);
            held
        });
        prop_assert_eq!(after.key, bi.key);
        prop_assert_eq!(after.pressed, bi.pressed);
        match action {
            Action::Press => {
                prop_assert!(!bi.pressed && !no_press);
                prop_assert!(modifiers_match(bi.key.modifiers, modifiers, strict));
                prop_assert!(bi.key.key != KEY_NONE || bi.key.modifiers != 0);
            }
            Action::Release => prop_assert!(bi.pressed),
            Action::None => {}
        }
        if bi.key.key == KEY_NONE {
            prop_assert_eq!(seen, None);
        } else if let Some(k) = seen {
            prop_assert_eq!(k, bi.key.key);
        }
    }

    /// The invariants of one inject: never a release, the pressed flag and
    /// key never changed, the latch only ever set to true, a press only for
    /// an unpressed binding whose modifiers match.
    #[test]
    fn inject_invariants(
        bi in any_binding(),
        event_modifiers in any_modifiers(),
        event_key in any_key(),
        pressed in any::<bool>(),
        strict in any::<bool>(),
    ) {
        let mut after = bi;
        let action = inject(&mut after, combo(event_modifiers, event_key), pressed, strict);
        prop_assert_ne!(action, Action::Release);
        prop_assert_eq!(after.key, bi.key);
        prop_assert_eq!(after.pressed, bi.pressed);
        prop_assert!(after.modifiers_match == bi.modifiers_match || after.modifiers_match);
        if action == Action::Press {
            prop_assert!(!bi.pressed);
            prop_assert!(modifiers_match(bi.key.modifiers, event_modifiers, strict));
        }
    }
}
