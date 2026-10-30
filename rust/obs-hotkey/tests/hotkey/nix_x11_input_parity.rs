//! Tier 3 parity: the shims against the original
//! `libobs/obs-nix-x11-input.c.h` (oracle/nix_x11_input.c).
//!
//! `test_nix_x11_input_steps` compiles the C into itself, so it never sees
//! the Rust swap (a variant linked against `libobs_rust-static` does not
//! link on its own: see `test/cmocka/tests.d/test_nix_x11_input_steps.cmake`).
//! This file is what compares the two.
//!
//! - The values: `OBS_KEY_LAST_VALUE`, the mouse key range,
//!   `XINPUT_MOUSE_LEN`, the three pointer masks and the size of a `bool`.
//! - Exhaustive over every key, `-2 .. OBS_KEY_LAST_VALUE + 2` and the
//!   `int` extremes: the keysym, and the XInput answer with the button
//!   pressed in only one slot, then held in only one slot, for every slot.
//! - The poll as the X11 platform runs it, on both sides with the same
//!   arrays: the C's raw event drain (a press sets the slot's pressed and
//!   update flags, a release its update flag), the answer of every mouse key
//!   and of the keys around them, then the commit, comparing the held array.
//!   Exhaustive over every drain of up to four press and release events of
//!   three slots (two neighbours and the last one a button fills), from
//!   each of the eight held states of those slots; a proptest over three
//!   polls in a row, with up to six events of any button each.
//! - The commit of every pressed, update and held state of a slot, in every
//!   slot at once, and of random arrays.
//! - The mask answer of every 16-bit mask for every mouse key and the keys
//!   around them.
//!
//! No intentional difference: nothing is excluded. Null arrays, which C
//! dereferences, are not passed.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use std::ffi::c_int;

use obs_c_oracle::nix_x11_input::{
    VALUE_NAMES, oracle_obs_nix_x11_input_commit, oracle_obs_nix_x11_input_keysym,
    oracle_obs_nix_x11_input_mask_pressed, oracle_obs_nix_x11_input_mouse_pressed,
    oracle_x11_input_value,
};
use obs_hotkey::ffi::nix_x11_input::{
    obs_nix_x11_input_commit, obs_nix_x11_input_keysym, obs_nix_x11_input_mask_pressed,
    obs_nix_x11_input_mouse_pressed,
};
use obs_hotkey::name_map::{KEY_NONE, OBS_KEY_LAST_VALUE, OBS_KEY_MOUSE1, OBS_KEY_MOUSE29};
use obs_hotkey::nix_x11_input::{BUTTON_MASK_1, BUTTON_MASK_2, BUTTON_MASK_3, MOUSE_LEN};
use proptest::prelude::*;

type Flags = [bool; MOUSE_LEN];

const LAST: c_int = OBS_KEY_LAST_VALUE as c_int;

/// Every key, the values just outside them and the `int` extremes.
fn every_key() -> impl Iterator<Item = c_int> {
    (-2..LAST + 2).chain([c_int::MIN, c_int::MAX])
}

/// The mouse keys and the two keys around them.
fn mouse_keys() -> impl Iterator<Item = c_int> {
    OBS_KEY_MOUSE1 - 1..=OBS_KEY_MOUSE29 + 1
}

fn rust_answer(key: c_int, pressed: &Flags, held: &Flags) -> bool {
    // SAFETY: both arrays hold MOUSE_LEN flags.
    unsafe { obs_nix_x11_input_mouse_pressed(key, pressed.as_ptr(), held.as_ptr()) }
}

fn c_answer(key: c_int, pressed: &Flags, held: &Flags) -> bool {
    // SAFETY: as above.
    unsafe { oracle_obs_nix_x11_input_mouse_pressed(key, pressed.as_ptr(), held.as_ptr()) }
}

fn rust_commit(update: &Flags, pressed: &Flags, held: &mut Flags) {
    // SAFETY: as above; `held` is a third array.
    unsafe { obs_nix_x11_input_commit(update.as_ptr(), pressed.as_ptr(), held.as_mut_ptr()) }
}

fn c_commit(update: &Flags, pressed: &Flags, held: &mut Flags) {
    // SAFETY: as above.
    unsafe { oracle_obs_nix_x11_input_commit(update.as_ptr(), pressed.as_ptr(), held.as_mut_ptr()) }
}

/// One raw button event, as the C drain stores it: button `n` (1 to 32).
#[derive(Debug, Clone, Copy)]
enum Event {
    Press(usize),
    Release(usize),
}

/// One poll on both sides: the drain, the answer of every mouse key and
/// the keys around them, the commit. Panics with the poll on a mismatch.
fn poll(events: &[Event], rust_held: &mut Flags, c_held: &mut Flags) {
    // The C clears both arrays, then the drain sets them.
    let mut pressed = [false; MOUSE_LEN];
    let mut update = [false; MOUSE_LEN];
    for event in events {
        match *event {
            Event::Press(n) => {
                pressed[n - 1] = true;
                update[n - 1] = true;
            }
            Event::Release(n) => update[n - 1] = true,
        }
    }
    for key in mouse_keys() {
        assert_eq!(
            rust_answer(key, &pressed, rust_held),
            c_answer(key, &pressed, c_held),
            "key {key} after {events:?}"
        );
    }
    rust_commit(&update, &pressed, rust_held);
    c_commit(&update, &pressed, c_held);
    assert_eq!(rust_held, c_held, "held after {events:?}");
}

#[test]
fn values_match_the_c() {
    let rust: [i64; 9] = [
        i64::try_from(OBS_KEY_LAST_VALUE).unwrap(),
        i64::from(KEY_NONE),
        i64::from(OBS_KEY_MOUSE1),
        i64::from(OBS_KEY_MOUSE29),
        i64::try_from(MOUSE_LEN).unwrap(),
        i64::from(BUTTON_MASK_1),
        i64::from(BUTTON_MASK_2),
        i64::from(BUTTON_MASK_3),
        i64::try_from(size_of::<bool>()).unwrap(),
    ];
    for (which, (name, want)) in VALUE_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain getter.
        let c = unsafe { oracle_x11_input_value(which) };
        assert_eq!(c, want, "{name}");
    }
}

#[test]
fn every_key_matches() {
    let mut named = 0;
    for key in every_key() {
        // SAFETY: plain values.
        let c = unsafe { oracle_obs_nix_x11_input_keysym(key) };
        assert_eq!(obs_nix_x11_input_keysym(key), c, "keysym of {key}");
        if c != 0 {
            named += 1;
        }

        for slot in 0..MOUSE_LEN {
            let mut one = [false; MOUSE_LEN];
            one[slot] = true;
            let none = [false; MOUSE_LEN];
            assert_eq!(
                rust_answer(key, &one, &none),
                c_answer(key, &one, &none),
                "key {key} pressed in slot {slot}"
            );
            assert_eq!(
                rust_answer(key, &none, &one),
                c_answer(key, &none, &one),
                "key {key} held in slot {slot}"
            );
        }
    }
    assert_eq!(named, 285, "keys with a keysym");
}

#[test]
fn every_short_drain_matches() {
    // Buttons 1 and 2 (slots 0 and 1: Mouse 1 and 3) and 32 (slot 31,
    // Mouse 28, the last one a button fills).
    const BUTTONS: [usize; 3] = [1, 2, 32];
    let symbols: Vec<Event> = BUTTONS
        .iter()
        .flat_map(|&n| [Event::Press(n), Event::Release(n)])
        .collect();

    let mut drains: Vec<Vec<Event>> = vec![Vec::new()];
    let mut last: Vec<Vec<Event>> = vec![Vec::new()];
    for _ in 0..4 {
        let next: Vec<Vec<Event>> = last
            .iter()
            .flat_map(|d| {
                symbols.iter().map(move |&e| {
                    let mut d = d.clone();
                    d.push(e);
                    d
                })
            })
            .collect();
        drains.extend(next.iter().cloned());
        last = next;
    }
    assert_eq!(drains.len(), 1 + 6 + 36 + 216 + 1296);

    for start in 0..8u8 {
        for drain in &drains {
            let mut held = [false; MOUSE_LEN];
            for (bit, &n) in BUTTONS.iter().enumerate() {
                held[n - 1] = start & (1 << bit) != 0;
            }
            let (mut rust_held, mut c_held) = (held, held);
            poll(drain, &mut rust_held, &mut c_held);
            // And the next poll, with no events: what is still held.
            poll(&[], &mut rust_held, &mut c_held);
        }
    }
}

#[test]
fn every_slot_state_commits_the_same() {
    // Slot i gets state (i + shift) % 8: bit 0 update, bit 1 pressed, bit 2
    // held. Eight shifts put every state in every slot.
    for shift in 0..8 {
        let state = |i: usize, bit: usize| ((i + shift) % 8) & (1 << bit) != 0;
        let update: Flags = std::array::from_fn(|i| state(i, 0));
        let pressed: Flags = std::array::from_fn(|i| state(i, 1));
        let held: Flags = std::array::from_fn(|i| state(i, 2));
        let (mut rust_held, mut c_held) = (held, held);
        rust_commit(&update, &pressed, &mut rust_held);
        c_commit(&update, &pressed, &mut c_held);
        assert_eq!(rust_held, c_held, "shift {shift}");
    }
}

#[test]
fn every_mask_matches() {
    for key in mouse_keys().chain([KEY_NONE, -1, LAST]) {
        for buttons in 0..=u16::MAX {
            // SAFETY: plain values.
            let c = unsafe { oracle_obs_nix_x11_input_mask_pressed(key, buttons) };
            assert_eq!(
                obs_nix_x11_input_mask_pressed(key, buttons),
                c,
                "key {key} mask {buttons:#06x}"
            );
        }
    }
}

fn flags() -> impl Strategy<Value = Flags> {
    proptest::collection::vec(any::<bool>(), MOUSE_LEN).prop_map(|v| Flags::try_from(v).unwrap())
}

fn event() -> impl Strategy<Value = Event> {
    (any::<bool>(), 1..=32usize).prop_map(|(press, n)| {
        if press {
            Event::Press(n)
        } else {
            Event::Release(n)
        }
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]

    #[test]
    fn three_polls_match(
        held in flags(),
        polls in proptest::collection::vec(proptest::collection::vec(event(), 0..=6), 3),
    ) {
        let (mut rust_held, mut c_held) = (held, held);
        for events in &polls {
            poll(events, &mut rust_held, &mut c_held);
        }
    }

    #[test]
    fn random_arrays_match(
        key in prop_oneof![OBS_KEY_MOUSE1 - 1..=OBS_KEY_MOUSE29 + 1, any::<c_int>()],
        update in flags(),
        pressed in flags(),
        held in flags(),
    ) {
        prop_assert_eq!(rust_answer(key, &pressed, &held), c_answer(key, &pressed, &held));
        let (mut rust_held, mut c_held) = (held, held);
        rust_commit(&update, &pressed, &mut rust_held);
        c_commit(&update, &pressed, &mut c_held);
        prop_assert_eq!(rust_held, c_held);
    }
}
