//! Tier 3: the original extracted C wrapper versus the shim on one recording
//! fixture, through the same real-named lock, callback-slot, unlock and
//! dstr_free adapters. Compares the full trace, the returned pointer and text
//! and allocation balance. No intentional differences. Mutants show the probe
//! catches each broken contract.
use core::ffi::{CStr, c_char, c_int};
use core::mem::{align_of, offset_of, size_of};
use obs_hotkey::ffi::combination_background_id::{
    ShortcutId, obs_hotkey_combination_get_background_id,
};
use obs_hotkey::ffi::nix_hotkey_state::obs_key_combination;
use obs_util::ffi::dstr::dstr;
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(obs_key_combination) -> *mut c_char;
unsafe extern "C" {
    fn oracle_hotkey_combination_background_id_probe(
        bits: u32,
        modifiers: u32,
        key: c_int,
        getter: Getter,
    ) -> u32;
    fn oracle_hotkey_combination_background_id_trace(
        bits: u32,
        getter: Option<Getter>,
        out: *mut c_char,
        size: usize,
    );
    fn oracle_hotkey_combination_background_id_layout(out: *mut usize);
    fn obs_hotkey_combination_background_id_lock() -> bool;
    fn obs_hotkey_combination_background_id_fields() -> *const Option<ShortcutId>;
    fn obs_hotkey_combination_background_id_unlock();
    fn obs_hotkey_combination_background_id_free(id: *mut dstr);
}

/// Bits: 1 lock succeeds, 2 a callback is registered, then the output kind
/// (none, empty, text, partial, failed empty, reenter) from bit 2.
const KINDS: u32 = 6;

fn probe(bits: u32, modifiers: u32, key: c_int, getter: Getter) -> u32 {
    // SAFETY: the oracle owns the whole fixture and passes the getter only a
    // combination; every adapter the getter reaches records into it.
    unsafe { oracle_hotkey_combination_background_id_probe(bits, modifiers, key, getter) }
}

fn check(bits: u32, modifiers: u32, key: c_int) {
    assert_eq!(
        probe(
            bits,
            modifiers,
            key,
            obs_hotkey_combination_get_background_id
        ),
        0,
        "bits={bits:#x} modifiers={modifiers:#x} key={key}"
    );
}

fn trace(bits: u32, getter: Option<Getter>) -> String {
    let mut out = [0 as c_char; 64];
    // SAFETY: the oracle writes a terminated trace of at most 63 bytes.
    unsafe {
        oracle_hotkey_combination_background_id_trace(bits, getter, out.as_mut_ptr(), out.len());
        CStr::from_ptr(out.as_ptr()).to_str().unwrap().to_owned()
    }
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 10];
    // SAFETY: the oracle writes ten size_t measurements from the real headers.
    unsafe { oracle_hotkey_combination_background_id_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<dstr>(),
            align_of::<dstr>(),
            offset_of!(dstr, array),
            offset_of!(dstr, len),
            offset_of!(dstr, capacity),
            size_of::<obs_key_combination>(),
            align_of::<obs_key_combination>(),
            offset_of!(obs_key_combination, modifiers),
            offset_of!(obs_key_combination, key),
            size_of::<Option<ShortcutId>>(),
        ]
    );
}

#[test]
fn original_traces_match_the_characterization() {
    // The C characterization's expected sequences, from the original itself.
    let kind = |k: u32| k << 2;
    for (bits, expected) in [
        (0, "L"),
        (2 | kind(3), "L"),
        (1, "LRUF"),
        (3, "LRUC"),
        (3 | kind(1), "LRUC"),
        (3 | kind(2), "LRUC"),
        (3 | kind(3), "LRUCF"),
        (3 | kind(4), "LRUCF"),
        (3 | kind(5), "LRUCLRUF"),
    ] {
        assert_eq!(trace(bits, None), expected, "bits={bits:#x}");
        assert_eq!(
            trace(bits, Some(obs_hotkey_combination_get_background_id)),
            expected,
            "bits={bits:#x}"
        );
    }
}

#[test]
fn characterized_edges() {
    let combinations = [(0, 0), ((1 << 2) | (1 << 1), 1), (u32::MAX, -1)];
    for bits in 0..4 * KINDS {
        for (modifiers, key) in combinations {
            check(bits, modifiers, key);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256), failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn get_background_id_matches_c(bits in 0..4 * KINDS, modifiers in any::<u32>(), key in any::<c_int>()) {
        check(bits, modifiers, key);
    }
}

fn empty() -> dstr {
    dstr {
        array: core::ptr::null_mut(),
        len: 0,
        capacity: 0,
    }
}

unsafe extern "C" fn calls_while_locked(combo: obs_key_combination) -> *mut c_char {
    // SAFETY: fixture adapters; the call deliberately happens before unlock.
    unsafe {
        if !obs_hotkey_combination_background_id_lock() {
            return core::ptr::null_mut();
        }
        let mut id = empty();
        let callback = *obs_hotkey_combination_background_id_fields();
        let ok = callback.is_some_and(|callback| callback(combo, &mut id));
        obs_hotkey_combination_background_id_unlock();
        if ok {
            return id.array;
        }
        obs_hotkey_combination_background_id_free(&mut id);
        core::ptr::null_mut()
    }
}

unsafe extern "C" fn reads_after_unlock(combo: obs_key_combination) -> *mut c_char {
    // SAFETY: fixture adapters; the slot is deliberately re-read after unlock.
    unsafe {
        if !obs_hotkey_combination_background_id_lock() {
            return core::ptr::null_mut();
        }
        let _ = *obs_hotkey_combination_background_id_fields();
        obs_hotkey_combination_background_id_unlock();
        let mut id = empty();
        let callback = *obs_hotkey_combination_background_id_fields();
        if callback.is_some_and(|callback| callback(combo, &mut id)) {
            return id.array;
        }
        obs_hotkey_combination_background_id_free(&mut id);
        core::ptr::null_mut()
    }
}

unsafe extern "C" fn leaks_partial(combo: obs_key_combination) -> *mut c_char {
    // SAFETY: fixture adapters; a failed call deliberately skips the free.
    unsafe {
        if !obs_hotkey_combination_background_id_lock() {
            return core::ptr::null_mut();
        }
        let callback = *obs_hotkey_combination_background_id_fields();
        obs_hotkey_combination_background_id_unlock();
        let mut id = empty();
        if callback.is_some_and(|callback| callback(combo, &mut id)) {
            id.array
        } else {
            core::ptr::null_mut()
        }
    }
}

unsafe extern "C" fn drops_result(combo: obs_key_combination) -> *mut c_char {
    // SAFETY: as the shim, but the identifier is deliberately freed.
    let id = unsafe { obs_hotkey_combination_get_background_id(combo) };
    // SAFETY: the shim returns a bfree-owned identifier or NULL.
    unsafe { obs_util::ffi::darray::bfree(id.cast()) };
    core::ptr::null_mut()
}

unsafe extern "C" fn reads_on_failed_lock(combo: obs_key_combination) -> *mut c_char {
    // SAFETY: fixture adapters; the slot is deliberately read without the lock.
    unsafe {
        if !obs_hotkey_combination_background_id_lock() {
            let _ = *obs_hotkey_combination_background_id_fields();
            return core::ptr::null_mut();
        }
        obs_hotkey_combination_background_id_unlock();
        obs_hotkey_combination_get_background_id(combo)
    }
}

unsafe extern "C" fn wrong_combo(combo: obs_key_combination) -> *mut c_char {
    let combo = obs_key_combination {
        modifiers: combo.modifiers ^ 1,
        key: combo.key,
    };
    // SAFETY: as the shim, with a deliberately altered combination.
    unsafe { obs_hotkey_combination_get_background_id(combo) }
}

#[test]
fn mutants_are_caught() {
    let text = 3 | (2 << 2);
    let partial = 3 | (3 << 2);
    let failed_lock = 2 | (2 << 2);
    assert_eq!(probe(text, 0, 1, calls_while_locked) & 1, 1);
    assert_eq!(probe(text, 0, 1, reads_after_unlock) & 1, 1);
    assert_eq!(probe(partial, 0, 1, leaks_partial) & 1, 1);
    assert_eq!(probe(text, 0, 1, drops_result) & 2, 2);
    assert_eq!(probe(failed_lock, 0, 1, reads_on_failed_lock) & 1, 1);
    assert_eq!(probe(text, 0, 1, wrong_combo) & 1, 1);
    for bits in [text, partial, failed_lock] {
        check(bits, 0, 1);
    }
}
