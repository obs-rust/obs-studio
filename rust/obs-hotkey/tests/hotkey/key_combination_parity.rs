//! Tier 3 parity: the `obs_key_combination_to_str` shim against the original
//! `libobs/obs-key-combination-host-posix.c.h`
//! (oracle/key_combination_host_posix.c).
//!
//! Both sides call `obs_key_to_str` by name; under cargo test that is the
//! oracle's stand-in, which calls [`hook`]. The hook records each call (the
//! key and the name's state on entry) and writes the name the case gives
//! that key: none, an empty string, or random bytes. Both sides get the
//! same random combination (any modifier bits, a key from the modifier
//! keys, `OBS_KEY_NONE`, the key table or any `int`), the same names, and
//! an output `dstr` that starts the same (zeroed, empty-but-allocated, or
//! holding random text). After each call this compares every byte of the
//! output (with its terminator), its length and capacity, and the hook's
//! calls. That each name's own `dstr` is freed is checked in Tier 1
//! (`key_combination.rs`), as `bnum_allocs` is process-wide.
//!
//! Intentional difference, not generated here: a null `str` does nothing
//! in Rust; C dereferences it once a key has a name.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

#![cfg(any(target_os = "linux", target_os = "freebsd"))]

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CString, c_int, c_void};

use obs_c_oracle::key_combination_host_posix::{
    OracleKeyCombination, oracle_key_combination_set_key_to_str, oracle_obs_key_combination_to_str,
};
use obs_hotkey::ffi::key_combination::obs_key_combination_to_str;
use obs_hotkey::ffi::nix_hotkey_state::obs_key_combination;
use obs_hotkey::name_map::{
    KEY_NONE, OBS_KEY_ALT, OBS_KEY_CONTROL, OBS_KEY_INSERT, OBS_KEY_LAST_VALUE, OBS_KEY_META,
    OBS_KEY_SHIFT,
};
use obs_util::ffi::darray::bfree;
use obs_util::ffi::dstr::{dstr, dstr_copy};
use proptest::prelude::*;

/// A `dstr`'s observable state: its bytes up to and including the
/// terminator (none for a null array), length and capacity.
type DstrState = (Option<Vec<u8>>, usize, usize);

thread_local! {
    /// The name each key gets; a key not in the map is left unnamed.
    static NAMES: RefCell<HashMap<c_int, CString>> = RefCell::new(HashMap::new());
    /// The hook's calls: the key and the name's state on entry.
    static CALLS: RefCell<Vec<(c_int, DstrState)>> = const { RefCell::new(Vec::new()) };
}

fn zeroed() -> dstr {
    dstr {
        array: std::ptr::null_mut(),
        len: 0,
        capacity: 0,
    }
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

/// The stand-in `obs_key_to_str`: records the call, then `dstr_copy`s the
/// key's name, as the platforms do.
unsafe extern "C" fn hook(key: c_int, str: *mut c_void) {
    let d = str.cast::<dstr>();
    // SAFETY: both sides pass a valid dstr.
    let entry = state(unsafe { &*d });
    CALLS.with(|c| c.borrow_mut().push((key, entry)));
    NAMES.with(|n| {
        if let Some(name) = n.borrow().get(&key) {
            // SAFETY: a valid dstr and a NUL-terminated string.
            unsafe { dstr_copy(d, name.as_ptr()) };
        }
    });
}

/// How the output starts.
#[derive(Debug, Clone)]
enum Start {
    Zeroed,
    /// Allocated but empty: `n` bytes copied in, then cut to length 0.
    Reserved(usize),
    Text(CString),
}

fn make(start: &Start) -> dstr {
    let mut d = zeroed();
    match start {
        Start::Zeroed => {}
        Start::Reserved(n) => {
            let filler = CString::new(vec![b'r'; *n]).unwrap();
            // SAFETY: a valid dstr and a NUL-terminated string.
            unsafe { dstr_copy(&mut d, filler.as_ptr()) };
            // SAFETY: the array holds `n` + 1 bytes.
            unsafe { *d.array = 0 };
            d.len = 0;
        }
        Start::Text(s) => {
            // SAFETY: a valid dstr and a NUL-terminated string.
            unsafe { dstr_copy(&mut d, s.as_ptr()) };
        }
    }
    d
}

/// One side's run: the output's final state and the hook's calls.
type Run = (DstrState, Vec<(c_int, DstrState)>);

fn run(side: impl Fn(&mut dstr), start: &Start) -> Run {
    CALLS.with(|c| c.borrow_mut().clear());
    let mut d = make(start);
    side(&mut d);
    let out = state(&d);
    free(&mut d);
    let calls = CALLS.with(|c| std::mem::take(&mut *c.borrow_mut()));
    (out, calls)
}

fn check(modifiers: u32, key: i32, names: &HashMap<c_int, CString>, start: &Start) -> (Run, Run) {
    NAMES.with(|n| *n.borrow_mut() = names.clone());
    // SAFETY: sets this thread's hook; `hook` matches the stand-in's type.
    unsafe { oracle_key_combination_set_key_to_str(Some(hook)) };
    let c = run(
        |d| {
            // SAFETY: a valid dstr.
            unsafe {
                oracle_obs_key_combination_to_str(
                    OracleKeyCombination { modifiers, key },
                    (d as *mut dstr).cast(),
                );
            }
        },
        start,
    );
    let rust = run(
        |d| {
            // SAFETY: a valid dstr.
            unsafe { obs_key_combination_to_str(obs_key_combination { modifiers, key }, d) };
        },
        start,
    );
    // SAFETY: clears this thread's hook.
    unsafe { oracle_key_combination_set_key_to_str(None) };
    (c, rust)
}

fn name_bytes() -> impl Strategy<Value = CString> {
    prop_oneof![
        Just(CString::default()),
        Just(CString::new(" ").unwrap()),
        Just(CString::new("\u{e9}").unwrap()),
        prop::collection::vec(1u8..=255, 1..12).prop_map(|b| CString::new(b).unwrap()),
    ]
}

fn key() -> impl Strategy<Value = i32> {
    let last = OBS_KEY_LAST_VALUE as i32;
    prop_oneof![
        Just(KEY_NONE),
        Just(OBS_KEY_CONTROL),
        Just(OBS_KEY_META),
        Just(OBS_KEY_ALT),
        Just(OBS_KEY_SHIFT),
        Just(OBS_KEY_INSERT),
        0..last,
        any::<i32>(),
    ]
}

fn names() -> impl Strategy<Value = HashMap<c_int, CString>> {
    prop::collection::vec((key(), prop::option::of(name_bytes())), 0..8).prop_map(|v| {
        v.into_iter()
            .filter_map(|(k, n)| n.map(|n| (k, n)))
            .collect()
    })
}

fn start() -> impl Strategy<Value = Start> {
    prop_oneof![
        Just(Start::Zeroed),
        (1usize..40).prop_map(Start::Reserved),
        prop::collection::vec(1u8..=255, 0..16).prop_map(|b| Start::Text(CString::new(b).unwrap())),
    ]
}

fn modifiers() -> impl Strategy<Value = u32> {
    prop_oneof![0u32..16, any::<u32>(), Just(u32::MAX)].prop_map(|m| {
        // Spread the low four bits over the four named modifiers so
        // every subset shows up often.
        let named = [OBS_CONTROL_BIT, OBS_META_BIT, OBS_ALT_BIT, OBS_SHIFT_BIT];
        named.iter().enumerate().fold(
            m & !0xf,
            |acc, (i, bit)| if m & (1 << i) != 0 { acc | bit } else { acc },
        )
    })
}

const OBS_CONTROL_BIT: u32 = obs_hotkey::nix_hotkey_state::INTERACT_CONTROL_KEY;
const OBS_META_BIT: u32 = obs_hotkey::nix_hotkey_state::INTERACT_COMMAND_KEY;
const OBS_ALT_BIT: u32 = obs_hotkey::nix_hotkey_state::INTERACT_ALT_KEY;
const OBS_SHIFT_BIT: u32 = obs_hotkey::nix_hotkey_state::INTERACT_SHIFT_KEY;

#[test]
fn every_modifier_subset_matches_c() {
    let names: HashMap<c_int, CString> = [
        (OBS_KEY_CONTROL, "C"),
        (OBS_KEY_META, "M"),
        (OBS_KEY_ALT, "A"),
        (OBS_KEY_SHIFT, "S"),
        (OBS_KEY_INSERT, "K"),
    ]
    .into_iter()
    .map(|(k, n)| (k, CString::new(n).unwrap()))
    .collect();
    for subset in 0u32..16 {
        let m = [OBS_CONTROL_BIT, OBS_META_BIT, OBS_ALT_BIT, OBS_SHIFT_BIT]
            .iter()
            .enumerate()
            .fold(0, |acc, (i, bit)| {
                if subset & (1 << i) != 0 {
                    acc | bit
                } else {
                    acc
                }
            });
        for key in [KEY_NONE, OBS_KEY_INSERT, OBS_KEY_CONTROL] {
            for start in [Start::Zeroed, Start::Text(CString::new("pre").unwrap())] {
                let (c, rust) = check(m, key, &names, &start);
                assert_eq!(rust, c, "modifiers {m:#x}, key {key}, start {start:?}");
            }
        }
    }
}

#[test]
fn no_hook_names_nothing_on_either_side() {
    // SAFETY: clears this thread's hook.
    unsafe { oracle_key_combination_set_key_to_str(None) };
    let start = Start::Text(CString::new("x").unwrap());
    let c = run(
        |d| {
            // SAFETY: a valid dstr.
            unsafe {
                oracle_obs_key_combination_to_str(
                    OracleKeyCombination {
                        modifiers: u32::MAX,
                        key: OBS_KEY_INSERT,
                    },
                    (d as *mut dstr).cast(),
                );
            }
        },
        &start,
    );
    let rust = run(
        |d| {
            // SAFETY: a valid dstr.
            unsafe {
                obs_key_combination_to_str(
                    obs_key_combination {
                        modifiers: u32::MAX,
                        key: OBS_KEY_INSERT,
                    },
                    d,
                );
            }
        },
        &start,
    );
    assert_eq!(rust, c);
    assert_eq!(c.0.0.as_deref(), Some(&b"x\0"[..]));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(512)))]

    #[test]
    fn random_combinations_match_c(
        modifiers in modifiers(),
        key in key(),
        names in names(),
        start in start(),
    ) {
        let (c, rust) = check(modifiers, key, &names, &start);
        prop_assert_eq!(rust, c);
    }
}
