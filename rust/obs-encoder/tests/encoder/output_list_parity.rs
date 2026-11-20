//! Tier 3: the original extracted `obs_encoder_add_output()` and
//! `obs_encoder_remove_output()` versus the shims on one recording fixture
//! encoder (oracle/encoder_outputs.c), through the real-named adapters.
//! Compared: every lock and unlock of `outputs_mutex` with the list each saw
//! (so a change outside the pair shows), nested or missing locks, each
//! replacement of the list's buffer, and the list and capacity left behind,
//! over any sequence of adds and removes of four outputs, NULL encoders and
//! NULL outputs included.
//!
//! The C walks the list with `darray_find()`, reading its length on every
//! step; the shim reads it once under the same lock. Nothing else runs under
//! the lock, so that is not observable. No intentional differences. Mutants
//! show the probe catches erasing every match, erasing the last match, an
//! append outside the lock, a missing NULL-output check and a removal that
//! never unlocks before returning.
#![allow(unsafe_code)]
use core::ffi::{CStr, c_char, c_void};
use obs_encoder::ffi::output_list::{obs_encoder_add_output, obs_encoder_remove_output};
use proptest::prelude::*;

type Change = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_encoder_outputs_probe(
        add: Change,
        remove: Change,
        ops: *const u8,
        count: usize,
    ) -> u32;
    fn oracle_encoder_outputs_original(
        ops: *const u8,
        count: usize,
        trace: *mut c_char,
        trace_size: usize,
        list: *mut c_char,
        list_size: usize,
    );
    fn oracle_encoder_outputs_entry_size() -> usize;
    fn oracle_encoder_outputs_adapters() -> bool;
    fn obs_encoder_outputs_lock(encoder: *mut c_void);
    fn obs_encoder_outputs_unlock(encoder: *mut c_void);
    fn obs_encoder_outputs_push(encoder: *mut c_void, output: *mut c_void);
    fn obs_encoder_outputs_count(encoder: *const c_void) -> usize;
    fn obs_encoder_outputs_at(encoder: *const c_void, index: usize) -> *mut c_void;
    fn obs_encoder_outputs_erase(encoder: *mut c_void, index: usize);
}

/// Operation bits, as the oracle reads them.
const NULL_OUTPUT: u8 = 0x04;
const NULL_ENCODER: u8 = 0x08;
const REMOVE: u8 = 0x10;

fn probe(add: Change, remove: Change, ops: &[u8]) -> u32 {
    // SAFETY: the oracle owns the fixture encoder and outputs every candidate
    // reaches; the slice outlives the call.
    unsafe { oracle_encoder_outputs_probe(add, remove, ops.as_ptr(), ops.len()) }
}

/// The original's trace and the list it leaves: `|` starts a call, `L` and
/// `U` are the lock and unlock with the list each saw (output digits, then
/// `]`), `A` a new buffer.
fn original(ops: &[u8]) -> (String, String) {
    let mut trace = [0 as c_char; 4096];
    let mut list = [0 as c_char; 64];
    // SAFETY: both buffers are writable for their sizes; the oracle writes
    // NUL-terminated text.
    unsafe {
        oracle_encoder_outputs_original(
            ops.as_ptr(),
            ops.len(),
            trace.as_mut_ptr(),
            trace.len(),
            list.as_mut_ptr(),
            list.len(),
        );
    }
    let text = |buffer: &[c_char]| {
        // SAFETY: NUL-terminated by the oracle.
        unsafe { CStr::from_ptr(buffer.as_ptr()) }
            .to_str()
            .unwrap()
            .to_owned()
    };
    (text(&trace), text(&list))
}

#[test]
fn real_header_adapters() {
    // SAFETY: the oracle checks the adapters on its own fixture.
    assert!(unsafe { oracle_encoder_outputs_adapters() });
    // SAFETY: a size query, no state.
    let entry = unsafe { oracle_encoder_outputs_entry_size() };
    assert_eq!(entry, size_of::<*mut c_void>());
}

#[test]
fn original_matches_the_characterization() {
    // append_and_remove_exact_output
    assert_eq!(
        original(&[0, 1, 2, REMOVE | 1, REMOVE, REMOVE | 2]),
        (
            "|L]U0]A|L0]U01]A|L01]U012]A|L012]U02]|L02]U2]|L2]U]".into(),
            String::new()
        )
    );
    // absent_output_is_unchanged: no new buffer for the absent removal.
    assert_eq!(
        original(&[REMOVE | 3, 0, 1, REMOVE | 3]),
        ("|L]U]|L]U0]A|L0]U01]A|L01]U01]".into(), "01".into())
    );
    // duplicates_remove_first_match_only
    assert_eq!(
        original(&[0, 1, 0, REMOVE, REMOVE, REMOVE]),
        (
            "|L]U0]A|L0]U01]A|L01]U010]A|L010]U10]|L10]U1]|L1]U1]".into(),
            "1".into()
        )
    );
    // null_arguments_do_not_lock_or_mutate
    assert_eq!(
        original(&[
            NULL_ENCODER | NULL_OUTPUT,
            REMOVE | NULL_ENCODER | NULL_OUTPUT,
            NULL_ENCODER,
            REMOVE | NULL_ENCODER,
            NULL_OUTPUT,
            REMOVE | NULL_OUTPUT,
            0,
            NULL_OUTPUT,
            REMOVE | NULL_OUTPUT,
        ]),
        ("|||||||L]U0]A||".into(), "0".into())
    );
}

const ALPHABET: [u8; 7] = [
    0,
    1,
    REMOVE,
    REMOVE | 1,
    NULL_OUTPUT,
    REMOVE | NULL_ENCODER,
    REMOVE | NULL_OUTPUT,
];

fn sequences(alphabet: &[u8], max: usize) -> Vec<Vec<u8>> {
    let mut all = vec![Vec::new()];
    let mut layer = vec![Vec::new()];
    for _ in 0..max {
        layer = layer
            .iter()
            .flat_map(|prefix: &Vec<u8>| {
                alphabet.iter().map(move |&value| {
                    let mut next = prefix.clone();
                    next.push(value);
                    next
                })
            })
            .collect();
        all.extend(layer.iter().cloned());
    }
    all
}

#[test]
fn characterized_edges() {
    for ops in sequences(&ALPHABET, 4) {
        assert_eq!(
            probe(obs_encoder_add_output, obs_encoder_remove_output, &ops),
            0,
            "{ops:?}"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(512), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn any_sequence_matches_c(ops in prop::collection::vec(0..32u8, 0..=32)) {
        prop_assert_eq!(probe(obs_encoder_add_output, obs_encoder_remove_output, &ops), 0);
    }
}

// ---------------------------------------------------------------------------
// mutants, over the same adapters

/// The unbroken steps, written directly over the adapters.
unsafe extern "C" fn add(encoder: *mut c_void, output: *mut c_void) {
    if encoder.is_null() || output.is_null() {
        return;
    }
    // SAFETY: mutants run only inside the probe, on its fixture encoder.
    unsafe {
        obs_encoder_outputs_lock(encoder);
        obs_encoder_outputs_push(encoder, output);
        obs_encoder_outputs_unlock(encoder);
    }
}

/// Indices of the entries equal to `output`, read under the caller's lock.
///
/// # Safety
/// The probe's fixture encoder, locked by the caller.
unsafe fn matches(encoder: *mut c_void, output: *mut c_void) -> Vec<usize> {
    // SAFETY: as the function's contract.
    unsafe {
        (0..obs_encoder_outputs_count(encoder))
            .filter(|&i| obs_encoder_outputs_at(encoder, i) == output)
            .collect()
    }
}

unsafe extern "C" fn remove(encoder: *mut c_void, output: *mut c_void) {
    if encoder.is_null() || output.is_null() {
        return;
    }
    // SAFETY: as in `add`.
    unsafe {
        obs_encoder_outputs_lock(encoder);
        if let Some(&first) = matches(encoder, output).first() {
            obs_encoder_outputs_erase(encoder, first);
        }
        obs_encoder_outputs_unlock(encoder);
    }
}

unsafe extern "C" fn erase_all_matches(encoder: *mut c_void, output: *mut c_void) {
    if encoder.is_null() || output.is_null() {
        return;
    }
    // SAFETY: as in `add`.
    unsafe {
        obs_encoder_outputs_lock(encoder);
        for index in matches(encoder, output).into_iter().rev() {
            obs_encoder_outputs_erase(encoder, index);
        }
        obs_encoder_outputs_unlock(encoder);
    }
}

unsafe extern "C" fn erase_last_match(encoder: *mut c_void, output: *mut c_void) {
    if encoder.is_null() || output.is_null() {
        return;
    }
    // SAFETY: as in `add`.
    unsafe {
        obs_encoder_outputs_lock(encoder);
        if let Some(&last) = matches(encoder, output).last() {
            obs_encoder_outputs_erase(encoder, last);
        }
        obs_encoder_outputs_unlock(encoder);
    }
}

unsafe extern "C" fn append_outside_lock(encoder: *mut c_void, output: *mut c_void) {
    if encoder.is_null() || output.is_null() {
        return;
    }
    // SAFETY: as in `add`.
    unsafe {
        obs_encoder_outputs_push(encoder, output);
        obs_encoder_outputs_lock(encoder);
        obs_encoder_outputs_unlock(encoder);
    }
}

unsafe extern "C" fn add_without_output_check(encoder: *mut c_void, output: *mut c_void) {
    if encoder.is_null() {
        return;
    }
    // SAFETY: as in `add`.
    unsafe {
        obs_encoder_outputs_lock(encoder);
        obs_encoder_outputs_push(encoder, output);
        obs_encoder_outputs_unlock(encoder);
    }
}

unsafe extern "C" fn remove_without_unlock(encoder: *mut c_void, output: *mut c_void) {
    if encoder.is_null() || output.is_null() {
        return;
    }
    // SAFETY: as in `add`; the probe records the mutex left held.
    unsafe {
        obs_encoder_outputs_lock(encoder);
        if let Some(&first) = matches(encoder, output).first() {
            obs_encoder_outputs_erase(encoder, first);
        }
    }
}

#[test]
fn mutants_are_caught() {
    let sweep = |add: Change, remove: Change| {
        sequences(&ALPHABET, 4)
            .iter()
            .fold(0, |diff, ops| diff | probe(add, remove, ops))
    };
    // The unbroken model of the steps passes the same sweep.
    assert_eq!(sweep(add, remove), 0);
    for (name, mutant_add, mutant_remove) in [
        (
            "erase_all_matches",
            add as Change,
            erase_all_matches as Change,
        ),
        ("erase_last_match", add, erase_last_match),
        ("append_outside_lock", append_outside_lock, remove),
        ("add_without_output_check", add_without_output_check, remove),
        ("remove_without_unlock", add, remove_without_unlock),
    ] {
        assert_ne!(sweep(mutant_add, mutant_remove), 0, "{name} survived");
    }
}
