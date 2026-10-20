//! Tier 2 layout and Tier 3 parity: the shims against the original
//! `libobs/obs-nix-hotkey-state.c.h` (oracle/nix_hotkey_state.c).
//!
//! The unchanged C tests `test_wayland_hotkeys`, `test_portal_shortcuts` and
//! `test_wayland_multiseat` compile the C into themselves, so they never see
//! the Rust swap; this file is what compares the two.
//!
//! Each run feeds one sequence of key, modifier, focus, shortcut, flush,
//! poll and free events to a C state and a Rust state, and after every step
//! compares the return value, the lock and unlock calls (both sides call the
//! counted stand-ins in `main.rs`), the focused keys and modifiers, and the
//! shortcut darray: item count, capacity and every item.
//!
//! Intentional difference, excluded here: a null `state` returns (false)
//! without locking in Rust; C dereferences it. Nothing else is excluded.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use std::ffi::c_void;
use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::nix_hotkey_state::{
    LAYOUT_NAMES, OracleKeyCombination, VALUE_NAMES, oracle_hotkey_state_layout,
    oracle_hotkey_state_value, oracle_obs_nix_hotkey_state_clear_keys,
    oracle_obs_nix_hotkey_state_flush, oracle_obs_nix_hotkey_state_free,
    oracle_obs_nix_hotkey_state_is_pressed, oracle_obs_nix_hotkey_state_set_key,
    oracle_obs_nix_hotkey_state_set_modifiers, oracle_obs_nix_hotkey_state_set_shortcut,
};
use obs_hotkey::ffi::nix_hotkey_state::{
    obs_key_combination, obs_nix_hotkey_shortcut, obs_nix_hotkey_state,
    obs_nix_hotkey_state_clear_keys, obs_nix_hotkey_state_flush, obs_nix_hotkey_state_free,
    obs_nix_hotkey_state_is_pressed, obs_nix_hotkey_state_set_key,
    obs_nix_hotkey_state_set_modifiers, obs_nix_hotkey_state_set_shortcut,
};
use obs_hotkey::name_map::{OBS_KEY_ALT, OBS_KEY_CONTROL, OBS_KEY_META, OBS_KEY_SHIFT};
use obs_hotkey::nix_hotkey_state::{
    INTERACT_ALT_KEY, INTERACT_COMMAND_KEY, INTERACT_CONTROL_KEY, INTERACT_SHIFT_KEY,
    OBS_KEY_LAST_VALUE,
};
use obs_util as _; // bmalloc
use proptest::prelude::*;

use super::hotkey_mutex;

const LAST: i32 = OBS_KEY_LAST_VALUE as i32;

#[test]
fn values_match_c() {
    let rust = [
        i64::try_from(OBS_KEY_LAST_VALUE).unwrap(),
        OBS_KEY_SHIFT.into(),
        OBS_KEY_CONTROL.into(),
        OBS_KEY_ALT.into(),
        OBS_KEY_META.into(),
        INTERACT_SHIFT_KEY.into(),
        INTERACT_CONTROL_KEY.into(),
        INTERACT_ALT_KEY.into(),
        INTERACT_COMMAND_KEY.into(),
    ];
    for (i, (name, value)) in VALUE_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain getter; `i` is in range.
        let c = unsafe { oracle_hotkey_state_value(i) };
        assert_eq!(value, c, "{name}");
    }
}

#[test]
fn layout_matches_c() {
    let rust = [
        size_of::<obs_key_combination>(),
        align_of::<obs_key_combination>(),
        offset_of!(obs_key_combination, modifiers),
        offset_of!(obs_key_combination, key),
        size_of::<obs_nix_hotkey_shortcut>(),
        align_of::<obs_nix_hotkey_shortcut>(),
        offset_of!(obs_nix_hotkey_shortcut, combo),
        offset_of!(obs_nix_hotkey_shortcut, observed),
        offset_of!(obs_nix_hotkey_shortcut, release_pending),
        size_of::<obs_nix_hotkey_state>(),
        align_of::<obs_nix_hotkey_state>(),
        offset_of!(obs_nix_hotkey_state, focused_modifiers),
        offset_of!(obs_nix_hotkey_state, shortcuts),
    ];
    for (i, (name, value)) in LAYOUT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain getter; `i` is in range.
        let c = unsafe { oracle_hotkey_state_layout(i) };
        assert_eq!(value, c, "{name}");
    }
    assert_eq!(size_of::<OracleKeyCombination>(), rust[0]);
    assert_eq!(offset_of!(obs_nix_hotkey_state, focused_keys), 0);
    // The darray is `struct darray`: array, num, capacity.
    assert_eq!(
        size_of::<obs_nix_hotkey_state>() - offset_of!(obs_nix_hotkey_state, shortcuts),
        3 * size_of::<usize>()
    );
}

#[derive(Debug, Clone, Copy)]
enum Op {
    SetKey(i32, bool),
    SetModifiers(u32),
    ClearKeys,
    SetShortcut(u32, i32, bool),
    Flush,
    IsPressed(i32),
    Free,
}

/// A call's result and its (lock, unlock) calls.
type Outcome = (bool, (u64, u64));

/// One state per side, zeroed as the C callers start it.
struct Pair {
    c: Box<obs_nix_hotkey_state>,
    rust: Box<obs_nix_hotkey_state>,
}

impl Pair {
    fn new() -> Self {
        Self {
            c: Box::default(),
            rust: Box::default(),
        }
    }

    fn c_ptr(&mut self) -> *mut c_void {
        (&raw mut *self.c).cast()
    }

    /// Runs `op` on both sides: the C outcome, then the Rust one.
    fn run(&mut self, op: Op) -> (Outcome, Outcome) {
        let c = self.c_ptr();
        let r = &raw mut *self.rust;
        let side = |f: &mut dyn FnMut() -> bool| {
            let before = hotkey_mutex::counts();
            let result = f();
            let after = hotkey_mutex::counts();
            (result, (after.0 - before.0, after.1 - before.1))
        };
        // SAFETY: both point to live, exclusive states with valid darrays;
        // only this thread uses them.
        unsafe {
            match op {
                Op::SetKey(key, pressed) => (
                    side(&mut || {
                        oracle_obs_nix_hotkey_state_set_key(c, key, pressed);
                        false
                    }),
                    side(&mut || {
                        obs_nix_hotkey_state_set_key(r, key, pressed);
                        false
                    }),
                ),
                Op::SetModifiers(m) => (
                    side(&mut || {
                        oracle_obs_nix_hotkey_state_set_modifiers(c, m);
                        false
                    }),
                    side(&mut || {
                        obs_nix_hotkey_state_set_modifiers(r, m);
                        false
                    }),
                ),
                Op::ClearKeys => (
                    side(&mut || {
                        oracle_obs_nix_hotkey_state_clear_keys(c);
                        false
                    }),
                    side(&mut || {
                        obs_nix_hotkey_state_clear_keys(r);
                        false
                    }),
                ),
                Op::SetShortcut(modifiers, key, active) => (
                    side(&mut || {
                        let combo = OracleKeyCombination { modifiers, key };
                        oracle_obs_nix_hotkey_state_set_shortcut(c, combo, active);
                        false
                    }),
                    side(&mut || {
                        let combo = obs_key_combination { modifiers, key };
                        obs_nix_hotkey_state_set_shortcut(r, combo, active);
                        false
                    }),
                ),
                Op::Flush => (
                    side(&mut || oracle_obs_nix_hotkey_state_flush(c)),
                    side(&mut || obs_nix_hotkey_state_flush(r)),
                ),
                Op::IsPressed(key) => (
                    side(&mut || oracle_obs_nix_hotkey_state_is_pressed(c, key)),
                    side(&mut || obs_nix_hotkey_state_is_pressed(r, key)),
                ),
                Op::Free => (
                    side(&mut || {
                        oracle_obs_nix_hotkey_state_free(c);
                        false
                    }),
                    side(&mut || {
                        obs_nix_hotkey_state_free(r);
                        false
                    }),
                ),
            }
        }
    }

    fn assert_same(&self, step: usize, op: Op) {
        let (c, r) = (&*self.c, &*self.rust);
        assert!(
            c.focused_keys == r.focused_keys,
            "step {step} {op:?}: focused keys differ"
        );
        assert_eq!(
            c.focused_modifiers, r.focused_modifiers,
            "step {step} {op:?}: modifiers"
        );
        let (cd, rd) = (c.shortcuts.raw(), r.shortcuts.raw());
        assert_eq!(
            (cd.num, cd.capacity),
            (rd.num, rd.capacity),
            "step {step} {op:?}: shortcut count and capacity"
        );
        assert_eq!(
            cd.array.is_null(),
            rd.array.is_null(),
            "step {step} {op:?}: shortcut buffer"
        );
        assert_eq!(
            c.shortcuts.items(),
            r.shortcuts.items(),
            "step {step} {op:?}: shortcuts"
        );
    }
}

impl Drop for Pair {
    fn drop(&mut self) {
        let c = self.c_ptr();
        // SAFETY: live states; free releases the darrays.
        unsafe {
            oracle_obs_nix_hotkey_state_free(c);
            obs_nix_hotkey_state_free(&raw mut *self.rust);
        }
    }
}

fn check(ops: &[Op]) {
    let mut pair = Pair::new();
    for (step, &op) in ops.iter().enumerate() {
        let (c, r) = pair.run(op);
        assert_eq!(r, c, "step {step} {op:?}: (result, (locks, unlocks))");
        pair.assert_same(step, op);
    }
}

/// The test_wayland_hotkeys and test_portal_shortcuts sequences, as the
/// key state sees them.
#[test]
fn cmocka_sequences_match_oracle() {
    let f13 = obs_hotkey::name_map::key_from_name(c"OBS_KEY_F13");
    let f14 = obs_hotkey::name_map::key_from_name(c"OBS_KEY_F14");
    let cs = INTERACT_CONTROL_KEY | INTERACT_SHIFT_KEY;
    let poll = Op::IsPressed;
    let cycle = [
        poll(OBS_KEY_SHIFT),
        poll(OBS_KEY_CONTROL),
        poll(OBS_KEY_ALT),
        poll(OBS_KEY_META),
        poll(f13),
    ];
    let mut ops = Vec::new();
    // Portal push-to-talk, with and without modifiers.
    for m in [0, cs] {
        ops.push(Op::SetShortcut(m, f13, true));
        ops.extend(cycle);
        ops.push(Op::SetShortcut(m, f13, false));
        ops.extend(cycle);
    }
    // A short tap: Activated and Deactivated before a poll, then flushes.
    ops.extend([
        Op::SetShortcut(0, f13, true),
        Op::SetShortcut(0, f13, false),
        Op::Flush,
    ]);
    ops.extend(cycle);
    ops.extend([Op::Flush, Op::Flush]);
    // Focused keys and modifiers, then focus loss.
    ops.extend([Op::SetModifiers(cs), Op::SetKey(f13, true)]);
    ops.extend(cycle);
    ops.extend([Op::SetModifiers(INTERACT_CONTROL_KEY), Op::ClearKeys]);
    ops.extend(cycle);
    ops.extend([poll(f14), Op::Free]);
    for key in -1..=LAST {
        ops.push(poll(key));
    }
    check(&ops);
}

/// Keys that collide often: the modifier keys, a few others, the bounds of
/// the valid range, and sometimes any code.
fn key() -> impl Strategy<Value = i32> {
    prop_oneof![
        6 => prop::sample::select(vec![
            OBS_KEY_SHIFT,
            OBS_KEY_CONTROL,
            OBS_KEY_ALT,
            OBS_KEY_META,
            1,
            30,
            31,
            100,
        ]),
        2 => prop::sample::select(vec![0, -1, LAST - 1, LAST, LAST + 1]),
        1 => any::<i32>(),
    ]
}

/// Mostly subsets of the four modifier bits, sometimes any bits.
fn modifiers() -> impl Strategy<Value = u32> {
    let bits = [
        INTERACT_SHIFT_KEY,
        INTERACT_CONTROL_KEY,
        INTERACT_ALT_KEY,
        INTERACT_COMMAND_KEY,
    ];
    prop_oneof![
        6 => (0u8..16).prop_map(move |set| {
            bits.iter()
                .enumerate()
                .filter(|(i, _)| set & (1 << i) != 0)
                .fold(0, |m, (_, b)| m | b)
        }),
        1 => any::<u32>(),
    ]
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => (key(), any::<bool>()).prop_map(|(k, p)| Op::SetKey(k, p)),
        2 => modifiers().prop_map(Op::SetModifiers),
        1 => Just(Op::ClearKeys),
        6 => (modifiers(), key(), any::<bool>()).prop_map(|(m, k, a)| Op::SetShortcut(m, k, a)),
        3 => Just(Op::Flush),
        6 => key().prop_map(Op::IsPressed),
        1 => Just(Op::Free),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn random_event_sequences_match_oracle(ops in prop::collection::vec(op(), 1..200)) {
        check(&ops);
    }
}

/// The one exclusion: a null state. Rust returns without locking; C is not
/// called.
#[test]
fn null_state_returns_without_locking() {
    let before = hotkey_mutex::counts();
    let null = std::ptr::null_mut();
    // SAFETY: null is the documented no-op.
    unsafe {
        obs_nix_hotkey_state_free(null);
        obs_nix_hotkey_state_set_key(null, 1, true);
        obs_nix_hotkey_state_set_modifiers(null, 1);
        obs_nix_hotkey_state_clear_keys(null);
        obs_nix_hotkey_state_set_shortcut(null, obs_key_combination::default(), true);
        assert!(!obs_nix_hotkey_state_flush(null));
        assert!(!obs_nix_hotkey_state_is_pressed(null, 1));
    }
    assert_eq!(hotkey_mutex::counts(), before);
}
