//! Tier 3 parity: the shims against the original
//! `libobs/obs-nix-portal-shortcut-ids.c.h` (oracle/portal_shortcut_ids.c).
//!
//! The unchanged C tests `test_portal_shortcut_ids` and
//! `test_portal_shortcuts` compile the C into themselves, so they never see
//! the Rust swap; this file is what compares the two.
//!
//! Both sides get the same random key combinations (any key code, in range
//! or not, and any modifier bits), the same id strings (built from known
//! and near-miss modifier names and key names, or arbitrary bytes), the
//! same key name callback behaviour, and an output `dstr` that starts the
//! same (zeroed, or holding random text). After each call this compares the
//! return value, every byte of the output (with its terminator), its length
//! and capacity, the `combo` out-value, and the key name callback's calls.
//! The `dstr` and key name functions both sides call are the Rust ports
//! (obs-util, obs-hotkey), which have their own parity tests.
//!
//! Intentional differences, excluded here: a null `id` or `trigger` dstr,
//! a null id string and a null `combo` out-pointer return false in Rust; C
//! dereferences them. Nothing else is excluded.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char, c_int, c_void};

use obs_c_oracle::portal_shortcut_ids::{
    OracleKeyCombination, OracleKeyNameFn, VALUE_NAMES, oracle_obs_nix_portal_shortcut_combo,
    oracle_obs_nix_portal_shortcut_id, oracle_obs_nix_portal_shortcut_trigger,
    oracle_portal_bindable, oracle_portal_modifier, oracle_portal_value,
};
use obs_hotkey::ffi::nix_hotkey_state::obs_key_combination;
use obs_hotkey::ffi::portal_shortcut_ids::{
    obs_nix_portal_key_name_func, obs_nix_portal_shortcut_combo, obs_nix_portal_shortcut_id,
    obs_nix_portal_shortcut_trigger,
};
use obs_hotkey::name_map::{
    KEY_NONE, OBS_KEY_LAST_VALUE, OBS_KEY_MOUSE1, OBS_KEY_MOUSE29, key_name,
};
use obs_hotkey::nix_hotkey_state::{
    INTERACT_ALT_KEY, INTERACT_COMMAND_KEY, INTERACT_CONTROL_KEY, INTERACT_SHIFT_KEY,
    KeyCombination,
};
use obs_hotkey::portal_shortcut_ids::{MODIFIER_NAMES, bindable};
use obs_util::ffi::darray::{bfree, bmalloc};
use obs_util::ffi::dstr::{dstr, dstr_copy};
use proptest::prelude::*;

const LAST: i32 = OBS_KEY_LAST_VALUE as i32;

#[test]
fn values_match_c() {
    let rust: [i64; 8] = [
        KEY_NONE.into(),
        LAST.into(),
        OBS_KEY_MOUSE1.into(),
        OBS_KEY_MOUSE29.into(),
        INTERACT_CONTROL_KEY.into(),
        INTERACT_ALT_KEY.into(),
        INTERACT_SHIFT_KEY.into(),
        INTERACT_COMMAND_KEY.into(),
    ];
    for (i, (name, value)) in VALUE_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain getter; `i` is in range.
        let c = unsafe { oracle_portal_value(i) };
        assert_eq!(value, c, "{name}");
    }
}

#[test]
fn modifier_table_matches_c() {
    let mut c_rows = Vec::new();
    for i in 0.. {
        let (mut modifier, mut id, mut trigger) = (0u32, std::ptr::null(), std::ptr::null());
        // SAFETY: three writable out-pointers.
        if !unsafe { oracle_portal_modifier(i, &mut modifier, &mut id, &mut trigger) } {
            break;
        }
        // SAFETY: the table's string literals.
        let (id, trigger) = unsafe { (CStr::from_ptr(id), CStr::from_ptr(trigger)) };
        c_rows.push((modifier, id.to_owned(), trigger.to_owned()));
    }
    let rust_rows: Vec<_> = MODIFIER_NAMES
        .iter()
        .map(|m| (m.modifier, m.id.to_owned(), m.trigger.to_owned()))
        .collect();
    assert_eq!(rust_rows, c_rows);
}

fn oracle_combo(modifiers: u32, key: i32) -> OracleKeyCombination {
    OracleKeyCombination { modifiers, key }
}

fn shim_combo(modifiers: u32, key: i32) -> obs_key_combination {
    obs_key_combination { modifiers, key }
}

#[test]
fn bindable_every_key_matches_c() {
    for key in (-300..LAST + 300).chain([i32::MIN, i32::MIN + 1, i32::MAX, i32::MAX - 1]) {
        for modifiers in [0, u32::MAX, INTERACT_CONTROL_KEY] {
            let combo = KeyCombination { modifiers, key };
            // SAFETY: a plain function of a value.
            let c = unsafe { oracle_portal_bindable(oracle_combo(modifiers, key)) };
            assert_eq!(bindable(combo), c, "{combo:?}");
        }
    }
}

// --------------------------------------------------------------------------
// dstr helpers

/// A `dstr`'s observable state: its bytes up to and including the
/// terminator (none for a null array), length and capacity.
type DstrState = (Option<Vec<u8>>, usize, usize);

fn zeroed() -> dstr {
    dstr {
        array: std::ptr::null_mut(),
        len: 0,
        capacity: 0,
    }
}

/// A dstr holding `start` (`dstr_copy`), or zeroed for `None`.
fn dstr_of(start: Option<&CString>) -> dstr {
    let mut d = zeroed();
    if let Some(s) = start {
        // SAFETY: a valid dstr and a NUL-terminated string.
        unsafe { dstr_copy(&mut d, s.as_ptr()) };
    }
    d
}

fn state(d: &dstr) -> DstrState {
    let bytes = (!d.array.is_null()).then(|| {
        // SAFETY: a non-null dstr array holds `len` bytes and a terminator.
        unsafe { std::slice::from_raw_parts(d.array as *const u8, d.len + 1) }.to_vec()
    });
    (bytes, d.len, d.capacity)
}

/// `dstr_free`.
fn free(d: &mut dstr) {
    // SAFETY: the array is `bmalloc`ed or null.
    unsafe { bfree(d.array.cast()) };
    *d = zeroed();
}

// --------------------------------------------------------------------------
// The key name callback

/// What the key name callback does.
#[derive(Debug, Clone)]
struct NameBehaviour {
    /// Bytes to put in the name (exactly, NULs included), or leave it.
    write: Option<Vec<u8>>,
    /// Its return value.
    ret: bool,
}

thread_local! {
    static BEHAVIOUR: RefCell<NameBehaviour> = const {
        RefCell::new(NameBehaviour { write: None, ret: false })
    };
    /// The keys the callback was called with, and the name's state on entry.
    static CALLS: RefCell<Vec<(c_int, DstrState)>> = const { RefCell::new(Vec::new()) };
}

/// Replaces `name` with exactly `bytes`, as a `bmalloc`ed dstr (a leading
/// or inner NUL included, which `dstr_copy` cannot make).
///
/// # Safety
///
/// `name` is a valid dstr.
unsafe fn put(name: *mut dstr, bytes: &[u8]) {
    // SAFETY: forwarded to the caller.
    let d = unsafe { &mut *name };
    free(d);
    if bytes.is_empty() {
        return;
    }
    // SAFETY: a fresh buffer of `len + 1` bytes.
    unsafe {
        let array = bmalloc(bytes.len() + 1).cast::<u8>();
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), array, bytes.len());
        *array.add(bytes.len()) = 0;
        d.array = array.cast::<c_char>();
    }
    d.len = bytes.len();
    d.capacity = bytes.len() + 1;
}

fn on_call(key: c_int, name: *mut dstr) -> bool {
    // SAFETY: both sides pass a valid dstr.
    let entry = state(unsafe { &*name });
    CALLS.with(|c| c.borrow_mut().push((key, entry)));
    let b = BEHAVIOUR.with(|b| b.borrow().clone());
    if let Some(bytes) = &b.write {
        // SAFETY: as above.
        unsafe { put(name, bytes) };
    }
    b.ret
}

unsafe extern "C" fn shim_key_name(key: c_int, name: *mut dstr) -> bool {
    on_call(key, name)
}

unsafe extern "C" fn oracle_key_name(key: c_int, name: *mut c_void) -> bool {
    on_call(key, name.cast())
}

fn take_calls() -> Vec<(c_int, DstrState)> {
    CALLS.with(|c| std::mem::take(&mut *c.borrow_mut()))
}

// --------------------------------------------------------------------------
// Strategies

fn key_strategy() -> impl Strategy<Value = i32> {
    prop_oneof![
        4 => KEY_NONE + 1..LAST,
        2 => -3..LAST + 3,
        1 => OBS_KEY_MOUSE1 - 1..=OBS_KEY_MOUSE29 + 1,
        1 => any::<i32>(),
    ]
}

fn modifier_strategy() -> impl Strategy<Value = u32> {
    let known = |set: u32| {
        MODIFIER_NAMES
            .iter()
            .enumerate()
            .filter(|(i, _)| set & (1 << i) != 0)
            .fold(0, |acc, (_, m)| acc | m.modifier)
    };
    prop_oneof![
        3 => (0u32..16).prop_map(known),
        2 => ((0u32..16), any::<u32>()).prop_map(move |(s, extra)| known(s) | (extra & 0xfff)),
        1 => any::<u32>(),
    ]
}

fn text_strategy() -> impl Strategy<Value = Option<CString>> {
    prop_oneof![
        Just(None),
        Just(Some(CString::new("").unwrap())),
        prop::collection::vec(1u8..=255, 1..80).prop_map(|v| Some(CString::new(v).unwrap())),
    ]
}

/// One segment of an id: known modifiers, their trigger spellings, near
/// misses, key names (bindable or not), and random bytes.
fn segment_strategy() -> impl Strategy<Value = Vec<u8>> {
    let words = [
        "ctrl",
        "alt",
        "shift",
        "super",
        "CTRL",
        "ALT",
        "SHIFT",
        "LOGO",
        "",
        "ctr",
        "ctrlx",
        "superr",
        "supe",
        "meta",
        "hyper",
        "Ctrl",
        " ctrl",
        "ctrl ",
        "OBS_KEY_",
        "OBS_KEY",
        "OBS_KEY_NONE",
        "OBS_KEY_LAST_VALUE",
        "OBS_KEY_MOUSE3",
        "obs_key_a",
        "OBS_KEY_a",
    ];
    prop_oneof![
        4 => prop::sample::select(words.to_vec()).prop_map(|w| w.as_bytes().to_vec()),
        4 => (0..LAST + 2).prop_map(|k| key_name(k).map_or(b"OBS_KEY_".to_vec(), |n| n.to_bytes().to_vec())),
        1 => prop::collection::vec(prop_oneof![b'a'..=b'z', Just(b'_'), Just(b'+'), 1u8..=255], 0..12),
    ]
}

fn id_strategy() -> impl Strategy<Value = CString> {
    let joined = prop::collection::vec(segment_strategy(), 0..6).prop_map(|parts| {
        let mut id = parts.join(&b'+');
        id.retain(|&b| b != 0);
        CString::new(id).unwrap()
    });
    prop_oneof![
        6 => joined,
        1 => prop::collection::vec(1u8..=255, 0..40).prop_map(|v| CString::new(v).unwrap()),
    ]
}

fn behaviour_strategy() -> impl Strategy<Value = Option<NameBehaviour>> {
    let write = prop_oneof![
        2 => Just(None),
        1 => Just(Some(Vec::new())),
        4 => prop::collection::vec(1u8..=255, 1..16).prop_map(Some),
        1 => prop::collection::vec(any::<u8>(), 1..16).prop_map(Some),
        1 => prop::collection::vec(1u8..=255, 0..8).prop_map(|v| {
            let mut n = vec![0];
            n.extend(v);
            Some(n)
        }),
    ];
    prop_oneof![
        1 => Just(None),
        8 => (write, prop::bool::weighted(0.8)).prop_map(|(write, ret)| Some(NameBehaviour { write, ret })),
    ]
}

// --------------------------------------------------------------------------
// The calls

/// `obs_nix_portal_shortcut_id` on both sides: (result, output) for C, then
/// for Rust.
fn run_id(modifiers: u32, key: i32, start: Option<&CString>) -> [(bool, DstrState); 2] {
    let mut c = dstr_of(start);
    let mut r = dstr_of(start);
    // SAFETY: two valid, exclusive dstrs.
    let (c_ret, r_ret) = unsafe {
        (
            oracle_obs_nix_portal_shortcut_id(oracle_combo(modifiers, key), (&raw mut c).cast()),
            obs_nix_portal_shortcut_id(shim_combo(modifiers, key), &raw mut r),
        )
    };
    let out = [(c_ret, state(&c)), (r_ret, state(&r))];
    free(&mut c);
    free(&mut r);
    out
}

/// `obs_nix_portal_shortcut_combo` on both sides: (result, out-value).
fn run_combo(id: &CStr) -> [(bool, (u32, i32)); 2] {
    let mut c = oracle_combo(0xdead_beef, LAST);
    let mut r = shim_combo(0xdead_beef, LAST);
    // SAFETY: a NUL-terminated id and two writable out-values.
    let (c_ret, r_ret) = unsafe {
        (
            oracle_obs_nix_portal_shortcut_combo(id.as_ptr(), &mut c),
            obs_nix_portal_shortcut_combo(id.as_ptr(), &mut r),
        )
    };
    [(c_ret, (c.modifiers, c.key)), (r_ret, (r.modifiers, r.key))]
}

type TriggerOutcome = (bool, DstrState, Vec<(c_int, DstrState)>);

/// `obs_nix_portal_shortcut_trigger` on both sides: (result, output,
/// callback calls).
fn run_trigger(
    modifiers: u32,
    key: i32,
    behaviour: Option<&NameBehaviour>,
    start: Option<&CString>,
) -> [TriggerOutcome; 2] {
    if let Some(b) = behaviour {
        BEHAVIOUR.with(|cell| *cell.borrow_mut() = b.clone());
    }
    let (c_fn, r_fn): (OracleKeyNameFn, obs_nix_portal_key_name_func) = match behaviour {
        Some(_) => (Some(oracle_key_name), Some(shim_key_name)),
        None => (None, None),
    };
    take_calls();

    let mut c = dstr_of(start);
    // SAFETY: a valid, exclusive dstr and a callback that takes one.
    let c_ret = unsafe {
        oracle_obs_nix_portal_shortcut_trigger(
            oracle_combo(modifiers, key),
            c_fn,
            (&raw mut c).cast(),
        )
    };
    let c_out = (c_ret, state(&c), take_calls());
    free(&mut c);

    let mut r = dstr_of(start);
    // SAFETY: as above.
    let r_ret =
        unsafe { obs_nix_portal_shortcut_trigger(shim_combo(modifiers, key), r_fn, &raw mut r) };
    let r_out = (r_ret, state(&r), take_calls());
    free(&mut r);
    [c_out, r_out]
}

#[test]
fn every_key_id_and_round_trip_match_c() {
    for key in -2..LAST + 2 {
        for modifiers in [0, INTERACT_SHIFT_KEY, u32::MAX] {
            let [c, r] = run_id(modifiers, key, None);
            assert_eq!(r, c, "key {key}, modifiers {modifiers:#x}");
            if let (true, (Some(bytes), ..)) = &c {
                let id = CStr::from_bytes_until_nul(bytes).unwrap();
                let [c, r] = run_combo(id);
                assert_eq!(r, c, "{id:?}");
                assert!(c.0, "{id:?} parses back");
            }
        }
    }
}

#[test]
fn characterized_trigger_cases_match_c() {
    let name = |write: &[u8], ret| NameBehaviour {
        write: Some(write.to_vec()),
        ret,
    };
    let first = KEY_NONE + 1;
    let start = CString::new("untouched").unwrap();
    for behaviour in [
        None,
        Some(name(b"F13", true)),
        Some(name(b"", true)),
        Some(name(b"leaked?", false)),
        Some(name(b"\0x", true)),
        Some(name(b"a\0b", true)),
        Some(NameBehaviour {
            write: None,
            ret: true,
        }),
    ] {
        for key in [first, KEY_NONE, OBS_KEY_MOUSE1, LAST, -1] {
            for start in [None, Some(&start)] {
                let [c, r] = run_trigger(INTERACT_CONTROL_KEY, key, behaviour.as_ref(), start);
                assert_eq!(r, c, "key {key}, {behaviour:?}, {start:?}");
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(2048)))]

    #[test]
    fn bindable_matches_c(modifiers in any::<u32>(), key in key_strategy()) {
        // SAFETY: a plain function of a value.
        let c = unsafe { oracle_portal_bindable(oracle_combo(modifiers, key)) };
        prop_assert_eq!(bindable(KeyCombination { modifiers, key }), c);
    }

    #[test]
    fn id_matches_c(
        modifiers in modifier_strategy(),
        key in key_strategy(),
        start in text_strategy(),
    ) {
        let [c, r] = run_id(modifiers, key, start.as_ref());
        prop_assert_eq!(r, c);
    }

    #[test]
    fn combo_matches_c(id in id_strategy()) {
        let [c, r] = run_combo(&id);
        prop_assert_eq!(r, c, "{:?}", id);
    }

    #[test]
    fn trigger_matches_c(
        modifiers in modifier_strategy(),
        key in key_strategy(),
        behaviour in behaviour_strategy(),
        start in text_strategy(),
    ) {
        let [c, r] = run_trigger(modifiers, key, behaviour.as_ref(), start.as_ref());
        prop_assert_eq!(r, c);
    }
}
