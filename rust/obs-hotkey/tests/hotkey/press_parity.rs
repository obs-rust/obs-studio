//! Tier 2 layout and Tier 3 parity: the shims against the original
//! `libobs/obs-hotkey-press.c.h` (oracle/hotkey_press.c).
//!
//! The unchanged C test `test_hotkey_press_state` sees the Rust swap through
//! a real libobs on Linux, and `test_hotkey_press_steps_rust` runs
//! `test_hotkey_press_steps` against the shims on every OS; this file
//! compares the two directly, over far more sequences than they script.
//!
//! Both sides run on the same fake bindings array. This file defines the
//! four hidden seams the C and the shims call (`obs_hotkey_binding_fields`,
//! `obs_hotkey_key_is_pressed`, `obs_hotkey_binding_press` and
//! `obs_hotkey_binding_release`) and logs every call, per thread: which
//! binding's fields were taken, which key was queried, which binding was
//! pressed or released and its latch at that moment (the callbacks run
//! inside, so the latch must already be written), and each hotkey callback (on the first press and the
//! last release of a hotkey's bindings, as libobs counts them). The key
//! state answers from a script first, then from the held keys, so a query
//! made at a different point, or one more or one less, changes the answers
//! as well as the log.
//!
//! A run feeds one sequence of polls (`query_hotkeys()`: the modifier query,
//! then every binding in order), polls with given modifiers, injects (every
//! binding in order), modifier queries and latch changes from outside to a
//! C world and a Rust world, and compares after every step: the call log
//! one to one (order and count), every binding's key, pressed flag and
//! latch, every hotkey's pressed count, and the returned modifiers.
//!
//! Intentional differences, excluded here: a null binding, or a null field
//! pointer from the accessor, does nothing in Rust; C dereferences it
//! (`null_binding_does_nothing` checks the Rust side). Dropping the C's
//! always-`NULL` `bool *pressed` parameter of `handle_binding()` is not a
//! difference: it was never part of an entry point. Nothing else is
//! excluded.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use std::cell::RefCell;
use std::collections::{BTreeSet, VecDeque};
use std::ffi::{c_int, c_void};
use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::hotkey_press::{
    LAYOUT_NAMES, OracleKeyCombination, VALUE_NAMES, oracle_hotkey_press_layout,
    oracle_hotkey_press_value, oracle_obs_hotkey_binding_inject, oracle_obs_hotkey_binding_query,
    oracle_obs_hotkeys_query_modifiers,
};
use obs_hotkey::ffi::nix_hotkey_state::obs_key_combination;
use obs_hotkey::ffi::press::{
    obs_hotkey_binding, obs_hotkey_binding_fields as Fields, obs_hotkey_binding_inject,
    obs_hotkey_binding_query, obs_hotkeys_query_modifiers,
};
use obs_hotkey::name_map::{
    KEY_NONE, OBS_KEY_ALT, OBS_KEY_CONTROL, OBS_KEY_LAST_VALUE, OBS_KEY_META, OBS_KEY_MOUSE1,
    OBS_KEY_MOUSE3, OBS_KEY_SHIFT,
};
use obs_hotkey::nix_hotkey_state::{
    INTERACT_ALT_KEY, INTERACT_COMMAND_KEY, INTERACT_CONTROL_KEY, INTERACT_SHIFT_KEY,
};
use obs_hotkey::press::MODIFIER_KEYS;
use proptest::prelude::*;

const CTRL: u32 = INTERACT_CONTROL_KEY;
const SHIFT: u32 = INTERACT_SHIFT_KEY;
const ALT: u32 = INTERACT_ALT_KEY;
const CMD: u32 = INTERACT_COMMAND_KEY;

/// `OBS_KEY_A` and `OBS_KEY_B` (obs-hotkeys.h order).
fn a() -> i32 {
    obs_hotkey::name_map::key_from_name(c"OBS_KEY_A")
}
fn b() -> i32 {
    obs_hotkey::name_map::key_from_name(c"OBS_KEY_B")
}

#[test]
fn values_match_c() {
    let rust = [
        i64::from(KEY_NONE),
        MODIFIER_KEYS[0].0.into(),
        MODIFIER_KEYS[1].0.into(),
        MODIFIER_KEYS[2].0.into(),
        MODIFIER_KEYS[3].0.into(),
        MODIFIER_KEYS[0].1.into(),
        MODIFIER_KEYS[1].1.into(),
        MODIFIER_KEYS[2].1.into(),
        MODIFIER_KEYS[3].1.into(),
    ];
    for (i, (name, value)) in VALUE_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain getter; `i` is in range.
        let c = unsafe { oracle_hotkey_press_value(i) };
        assert_eq!(value, c, "{name}");
    }
    assert_eq!(
        MODIFIER_KEYS.map(|(k, _)| k),
        [OBS_KEY_SHIFT, OBS_KEY_CONTROL, OBS_KEY_ALT, OBS_KEY_META]
    );
}

#[test]
fn layout_matches_c() {
    let rust = [
        size_of::<Fields>(),
        align_of::<Fields>(),
        offset_of!(Fields, key),
        offset_of!(Fields, pressed),
        offset_of!(Fields, modifiers_match),
        size_of::<obs_key_combination>(),
        align_of::<obs_key_combination>(),
        offset_of!(obs_key_combination, modifiers),
        offset_of!(obs_key_combination, key),
    ];
    for (i, (name, value)) in LAYOUT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain getter; `i` is in range.
        let c = unsafe { oracle_hotkey_press_layout(i) };
        assert_eq!(value, c, "{name}");
    }
}

// ---------------------------------------------------------------------------
// The fake libobs side

/// One binding of the fake array. The seams reach it through the pointer
/// the entry points get, as libobs's reach `struct obs_hotkey_binding`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FakeBinding {
    key: obs_key_combination,
    pressed: bool,
    modifiers_match: bool,
    /// Its index in the array, for the log.
    index: usize,
    /// Its hotkey, an index into `Seams::hotkeys`.
    hotkey: usize,
}

/// One call into the seams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Call {
    Fields(usize),
    IsPressed(i32),
    /// A press or release, with the binding's latch as the callback sees
    /// it (a callback runs inside, and may look at the bindings).
    Press(usize, bool),
    Release(usize, bool),
    /// A hotkey's callback (first press, last release): hotkey, pressed.
    Callback(usize, bool),
}

#[derive(Default)]
struct Seams {
    log: Vec<Call>,
    held: BTreeSet<i32>,
    /// Answers given before the held keys are looked at, one per query.
    script: VecDeque<bool>,
    /// `obs_hotkey.pressed` of each hotkey.
    hotkeys: Vec<i32>,
}

thread_local! {
    static SEAMS: RefCell<Seams> = RefCell::new(Seams::default());
}

fn with_seams<R>(f: impl FnOnce(&mut Seams) -> R) -> R {
    SEAMS.with(|s| f(&mut s.borrow_mut()))
}

/// The binding accessor (`obs-hotkey.c.h`).
///
/// # Safety
///
/// `binding` points at a live [`FakeBinding`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_binding_fields(binding: *mut obs_hotkey_binding) -> Fields {
    let b = binding.cast::<FakeBinding>();
    // SAFETY: a live FakeBinding, per the caller.
    let index = unsafe { (*b).index };
    with_seams(|s| s.log.push(Call::Fields(index)));
    // SAFETY: as above; the pointers are to its own fields.
    unsafe {
        Fields {
            key: &raw const (*b).key,
            pressed: &raw const (*b).pressed,
            modifiers_match: &raw mut (*b).modifiers_match,
        }
    }
}

/// The platform key state: the script, then the held keys.
#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_key_is_pressed(key: c_int) -> bool {
    with_seams(|s| {
        s.log.push(Call::IsPressed(key));
        s.script
            .pop_front()
            .unwrap_or_else(|| s.held.contains(&key))
    })
}

/// `press_released_binding()`, without the routing.
///
/// # Safety
///
/// `binding` points at a live [`FakeBinding`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_binding_press(binding: *mut obs_hotkey_binding) {
    let b = binding.cast::<FakeBinding>();
    // SAFETY: a live FakeBinding, per the caller.
    let (index, hotkey, latch) = unsafe {
        (*b).pressed = true;
        ((*b).index, (*b).hotkey, (*b).modifiers_match)
    };
    with_seams(|s| {
        s.log.push(Call::Press(index, latch));
        let count = &mut s.hotkeys[hotkey];
        let was = *count;
        *count = count.wrapping_add(1);
        if was == 0 {
            s.log.push(Call::Callback(hotkey, true));
        }
    });
}

/// `release_pressed_binding()`, without the routing.
///
/// # Safety
///
/// `binding` points at a live [`FakeBinding`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_binding_release(binding: *mut obs_hotkey_binding) {
    let b = binding.cast::<FakeBinding>();
    // SAFETY: a live FakeBinding, per the caller.
    let (index, hotkey, latch) = unsafe {
        (*b).pressed = false;
        ((*b).index, (*b).hotkey, (*b).modifiers_match)
    };
    with_seams(|s| {
        s.log.push(Call::Release(index, latch));
        let count = &mut s.hotkeys[hotkey];
        *count = count.wrapping_sub(1);
        if *count == 0 {
            s.log.push(Call::Callback(hotkey, false));
        }
    });
}

// ---------------------------------------------------------------------------
// The two sides

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    C,
    Rust,
}

/// One binding to set up: its combination, its hotkey and its latch.
#[derive(Debug, Clone, Copy)]
struct Spec {
    modifiers: u32,
    key: i32,
    hotkey: usize,
    latch: bool,
}

/// One step of a run.
#[derive(Debug, Clone)]
enum Op {
    /// `query_hotkeys()`: hold `held`, answer the first queries from
    /// `script`, query the modifiers, then every binding.
    Poll {
        held: Vec<i32>,
        script: Vec<bool>,
        no_press: bool,
        strict: bool,
    },
    /// Every binding with the given modifiers (no modifier query).
    PollWith {
        held: Vec<i32>,
        script: Vec<bool>,
        modifiers: u32,
        no_press: bool,
        strict: bool,
    },
    /// The walk of `obs_hotkey_inject_event()`.
    Inject {
        modifiers: u32,
        key: i32,
        pressed: bool,
        strict: bool,
    },
    /// `obs_hotkeys_query_modifiers()` alone.
    QueryModifiers { held: Vec<i32>, script: Vec<bool> },
    /// A latch set from outside, as test_hotkey_press_steps's
    /// `binding_fields` does through the accessor.
    SetLatch { index: usize, value: bool },
}

/// What a step leaves behind.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Outcome {
    log: Vec<Call>,
    modifiers: Option<u32>,
    bindings: Vec<FakeBinding>,
    hotkeys: Vec<i32>,
    script_left: usize,
}

fn query(side: Side, b: *mut FakeBinding, modifiers: u32, no_press: bool, strict: bool) {
    let b = b.cast::<obs_hotkey_binding>();
    match side {
        // SAFETY: a live FakeBinding of this run.
        Side::C => unsafe {
            oracle_obs_hotkey_binding_query(b.cast(), modifiers, no_press, strict)
        },
        // SAFETY: as above.
        Side::Rust => unsafe { obs_hotkey_binding_query(b, modifiers, no_press, strict) },
    }
}

fn inject(
    side: Side,
    b: *mut FakeBinding,
    combo: obs_key_combination,
    pressed: bool,
    strict: bool,
) {
    let b = b.cast::<obs_hotkey_binding>();
    match side {
        // SAFETY: a live FakeBinding of this run.
        Side::C => unsafe {
            oracle_obs_hotkey_binding_inject(
                b.cast::<c_void>(),
                OracleKeyCombination {
                    modifiers: combo.modifiers,
                    key: combo.key,
                },
                pressed,
                strict,
            )
        },
        // SAFETY: as above.
        Side::Rust => unsafe { obs_hotkey_binding_inject(b, combo, pressed, strict) },
    }
}

fn query_modifiers(side: Side) -> u32 {
    match side {
        // SAFETY: calls only the seams above.
        Side::C => unsafe { oracle_obs_hotkeys_query_modifiers() },
        Side::Rust => obs_hotkeys_query_modifiers(),
    }
}

fn set_keys(held: &[i32], script: &[bool]) {
    with_seams(|s| {
        s.held = held.iter().copied().collect();
        s.script = script.iter().copied().collect();
    });
}

/// Runs `ops` on one side and returns the outcome after each step.
fn run(side: Side, specs: &[Spec], hotkeys: usize, ops: &[Op]) -> Vec<Outcome> {
    let mut bindings: Vec<FakeBinding> = specs
        .iter()
        .enumerate()
        .map(|(index, s)| FakeBinding {
            key: obs_key_combination {
                modifiers: s.modifiers,
                key: s.key,
            },
            pressed: false,
            modifiers_match: s.latch,
            index,
            hotkey: s.hotkey,
        })
        .collect();
    with_seams(|s| {
        *s = Seams {
            hotkeys: vec![0; hotkeys],
            ..Seams::default()
        }
    });
    let base = bindings.as_mut_ptr();
    let n = bindings.len();
    let mut out = Vec::with_capacity(ops.len());
    for op in ops {
        let mut modifiers = None;
        match op {
            Op::Poll {
                held,
                script,
                no_press,
                strict,
            } => {
                set_keys(held, script);
                let m = query_modifiers(side);
                modifiers = Some(m);
                for i in 0..n {
                    // SAFETY: `i` is in range of the array, which does not
                    // move during the run.
                    query(side, unsafe { base.add(i) }, m, *no_press, *strict);
                }
            }
            Op::PollWith {
                held,
                script,
                modifiers: m,
                no_press,
                strict,
            } => {
                set_keys(held, script);
                for i in 0..n {
                    // SAFETY: as above.
                    query(side, unsafe { base.add(i) }, *m, *no_press, *strict);
                }
            }
            Op::Inject {
                modifiers: m,
                key,
                pressed,
                strict,
            } => {
                let combo = obs_key_combination {
                    modifiers: *m,
                    key: *key,
                };
                for i in 0..n {
                    // SAFETY: as above.
                    inject(side, unsafe { base.add(i) }, combo, *pressed, *strict);
                }
            }
            Op::QueryModifiers { held, script } => {
                set_keys(held, script);
                modifiers = Some(query_modifiers(side));
            }
            Op::SetLatch { index, value } => {
                if *index < n {
                    // SAFETY: as above.
                    unsafe { (*base.add(*index)).modifiers_match = *value };
                }
            }
        }
        let (log, hotkeys, script_left) = with_seams(|s| {
            (
                std::mem::take(&mut s.log),
                s.hotkeys.clone(),
                s.script.len(),
            )
        });
        out.push(Outcome {
            log,
            modifiers,
            bindings: bindings.clone(),
            hotkeys,
            script_left,
        });
    }
    out
}

/// Runs both sides and compares them step by step.
fn check(specs: &[Spec], hotkeys: usize, ops: &[Op]) -> Result<(), TestCaseError> {
    let c = run(Side::C, specs, hotkeys, ops);
    let rust = run(Side::Rust, specs, hotkeys, ops);
    for (i, (c, rust)) in c.iter().zip(&rust).enumerate() {
        prop_assert_eq!(c, rust, "step {}: {:?}", i, &ops[i]);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Directed cases

fn spec(modifiers: u32, key: i32, hotkey: usize) -> Spec {
    Spec {
        modifiers,
        key,
        hotkey,
        latch: false,
    }
}

fn poll(held: &[i32], no_press: bool, strict: bool) -> Op {
    Op::Poll {
        held: held.to_vec(),
        script: Vec::new(),
        no_press,
        strict,
    }
}

/// The bindings of test_hotkey_press_steps and test_hotkey_press_state:
/// Ctrl+A, A, a modifier-only Ctrl, an empty binding, Ctrl+Shift+A, two
/// bindings of one hotkey (A and the middle mouse button), a duplicate and
/// two hotkeys on one key.
fn characterized_specs() -> Vec<Spec> {
    vec![
        spec(CTRL, a(), 0),
        spec(0, a(), 1),
        spec(CTRL, KEY_NONE, 2),
        spec(0, KEY_NONE, 3),
        spec(CTRL | SHIFT, a(), 4),
        spec(0, b(), 5),
        spec(0, OBS_KEY_MOUSE3, 5),
        spec(CTRL, a(), 0),
        spec(ALT, b(), 6),
    ]
}

/// Press, modifier change and release in every order: every permutation
/// of Ctrl down, A down, Ctrl up, A up, Shift down and Shift up, with a
/// poll after each, strict and not, with presses on and off, and an
/// inject of each event as it happens.
#[test]
fn every_press_order_matches_c() {
    let ctrl = OBS_KEY_CONTROL;
    let shift = OBS_KEY_SHIFT;
    let events: [(i32, bool); 6] = [
        (ctrl, true),
        (a(), true),
        (ctrl, false),
        (a(), false),
        (shift, true),
        (shift, false),
    ];
    let specs = characterized_specs();
    let mut order: Vec<usize> = (0..events.len()).collect();
    let mut runs = 0;
    permute(&mut order, 0, &mut |order| {
        for strict in [true, false] {
            for no_press in [false, true] {
                for injects in [false, true] {
                    let mut held = BTreeSet::new();
                    let mut ops = Vec::new();
                    for &e in order {
                        let (key, down) = events[e];
                        if down {
                            held.insert(key);
                        } else {
                            held.remove(&key);
                        }
                        let keys: Vec<i32> = held.iter().copied().collect();
                        if injects {
                            let modifiers = MODIFIER_KEYS
                                .iter()
                                .filter(|(k, _)| held.contains(k))
                                .fold(0, |m, (_, bit)| m | bit);
                            ops.push(Op::Inject {
                                modifiers,
                                key,
                                pressed: down,
                                strict,
                            });
                        }
                        ops.push(poll(&keys, no_press, strict));
                        ops.push(poll(&keys, no_press, strict));
                    }
                    check(&specs, 7, &ops).unwrap();
                    runs += 1;
                }
            }
        }
    });
    assert_eq!(runs, 720 * 8);
}

fn permute(v: &mut Vec<usize>, k: usize, f: &mut impl FnMut(&[usize])) {
    if k == v.len() {
        f(v);
        return;
    }
    for i in k..v.len() {
        v.swap(k, i);
        permute(v, k + 1, f);
        v.swap(k, i);
    }
}

/// The scripted sequences of test_hotkey_press_steps, on both sides.
#[test]
fn characterized_sequences_match_c() {
    let specs = characterized_specs();
    let ctrl = OBS_KEY_CONTROL;
    let ops = vec![
        Op::QueryModifiers {
            held: vec![OBS_KEY_SHIFT, OBS_KEY_ALT, a()],
            script: vec![],
        },
        Op::QueryModifiers {
            held: vec![ctrl, OBS_KEY_META],
            script: vec![],
        },
        poll(&[ctrl, a()], false, true),
        poll(&[ctrl, a()], false, true),
        poll(&[a()], false, true),
        poll(&[ctrl], false, true),
        poll(&[ctrl, a()], true, true),
        poll(&[ctrl, a()], false, true),
        poll(&[ctrl, OBS_KEY_SHIFT, a()], false, false),
        poll(&[ctrl, OBS_KEY_SHIFT, OBS_KEY_ALT, a(), b()], false, false),
        Op::Inject {
            modifiers: CTRL | SHIFT,
            key: b(),
            pressed: true,
            strict: false,
        },
        Op::Inject {
            modifiers: 0,
            key: b(),
            pressed: false,
            strict: true,
        },
        Op::Inject {
            modifiers: CTRL | ALT,
            key: KEY_NONE,
            pressed: false,
            strict: false,
        },
        Op::Inject {
            modifiers: 0,
            key: OBS_KEY_MOUSE3,
            pressed: true,
            strict: true,
        },
        Op::SetLatch {
            index: 0,
            value: true,
        },
        poll(&[ctrl, a()], false, true),
        poll(&[OBS_KEY_MOUSE3], false, true),
        poll(&[], true, true),
        poll(&[], false, false),
    ];
    check(&specs, 7, &ops).unwrap();
}

/// A null binding does nothing in Rust: no seam is called (C dereferences
/// it, so this is not compared).
#[test]
fn null_binding_does_nothing() {
    with_seams(|s| *s = Seams::default());
    // SAFETY: null is what is being checked.
    unsafe {
        obs_hotkey_binding_query(std::ptr::null_mut(), 0, false, true);
        obs_hotkey_binding_inject(
            std::ptr::null_mut(),
            obs_key_combination::default(),
            true,
            true,
        );
    }
    assert!(with_seams(|s| s.log.is_empty()));
}

// ---------------------------------------------------------------------------
// Random runs

/// Keys: none, the modifiers, letters, the mouse buttons (MOUSE1 is
/// unbindable in libobs but the state machine takes it), any code in range,
/// and any int.
fn any_key() -> impl Strategy<Value = i32> {
    prop_oneof![
        3 => Just(KEY_NONE),
        2 => prop::sample::select(vec![
            OBS_KEY_SHIFT,
            OBS_KEY_CONTROL,
            OBS_KEY_ALT,
            OBS_KEY_META,
            OBS_KEY_MOUSE1,
            OBS_KEY_MOUSE3,
        ]),
        4 => prop::sample::select(vec![a(), b()]),
        1 => 0..OBS_KEY_LAST_VALUE as i32 + 2,
        1 => any::<i32>(),
    ]
}

/// The four modifier bits in any combination, sometimes any bits at all.
fn any_modifiers() -> impl Strategy<Value = u32> {
    prop_oneof![
        8 => (0u32..16).prop_map(|m| {
            [SHIFT, CTRL, ALT, CMD]
                .iter()
                .enumerate()
                .filter(|(i, _)| m & (1 << i) != 0)
                .fold(0, |acc, (_, bit)| acc | bit)
        }),
        1 => any::<u32>(),
    ]
}

fn any_held() -> impl Strategy<Value = Vec<i32>> {
    prop::collection::vec(
        prop::sample::select(vec![
            OBS_KEY_SHIFT,
            OBS_KEY_CONTROL,
            OBS_KEY_ALT,
            OBS_KEY_META,
            OBS_KEY_MOUSE1,
            OBS_KEY_MOUSE3,
            a(),
            b(),
        ]),
        0..5,
    )
}

fn any_script() -> impl Strategy<Value = Vec<bool>> {
    prop_oneof![
        3 => Just(Vec::new()),
        1 => prop::collection::vec(any::<bool>(), 0..8),
    ]
}

fn any_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        6 => (any_held(), any_script(), any::<bool>(), any::<bool>()).prop_map(
            |(held, script, no_press, strict)| Op::Poll { held, script, no_press, strict }
        ),
        1 => (any_held(), any_script(), any_modifiers(), any::<bool>(), any::<bool>()).prop_map(
            |(held, script, modifiers, no_press, strict)| Op::PollWith {
                held,
                script,
                modifiers,
                no_press,
                strict,
            }
        ),
        3 => (any_modifiers(), any_key(), any::<bool>(), any::<bool>()).prop_map(
            |(modifiers, key, pressed, strict)| Op::Inject { modifiers, key, pressed, strict }
        ),
        1 => (any_held(), any_script())
            .prop_map(|(held, script)| Op::QueryModifiers { held, script }),
        1 => (0usize..8, any::<bool>()).prop_map(|(index, value)| Op::SetLatch { index, value }),
    ]
}

fn any_spec(hotkeys: usize) -> impl Strategy<Value = Spec> {
    (any_modifiers(), any_key(), 0..hotkeys, any::<bool>()).prop_map(
        |(modifiers, key, hotkey, latch)| Spec {
            modifiers,
            key,
            hotkey,
            latch,
        },
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    /// Random binding sets (several bindings of one hotkey included),
    /// starting latches, and sequences of polls, injects, modifier queries
    /// and latch changes, with scripted key states.
    #[test]
    fn random_sequences_match_c(
        specs in prop::collection::vec(any_spec(3), 0..8),
        ops in prop::collection::vec(any_op(), 1..40),
    ) {
        check(&specs, 3, &ops)?;
    }
}
