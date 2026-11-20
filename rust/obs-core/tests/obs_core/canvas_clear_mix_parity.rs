//! Tier 3: the original extracted `obs_canvas_clear_mix()` versus the shim
//! on one recording fixture core (oracle/canvas_clear_mix.c), through the
//! real-named slot, mutex, list and teardown adapters. Compared: the trace
//! of every core read, lock, unlock and free (with its checks that the free
//! runs under the lock after the erase while the canvas still holds the mix,
//! and that the canvas is still attached at the unlock), the list left
//! behind, the detach, and the untouched bytes of the canvas, its weak
//! references, its source and every mix. Lists: empty, one, many, absent,
//! NULL entries and duplicate pointers.
//!
//! Excluded: a NULL canvas (a NULL dereference in both; Tier 2's
//! characterization probes the fault) and a NULL core with a mix (likewise).
//! No intentional differences. Mutants show the probe catches each broken
//! step.
use core::ffi::{CStr, c_char, c_void};
use core::ptr::null_mut;
use obs_core::ffi::canvas_clear_mix::obs_canvas_clear_mix;
use proptest::prelude::*;

const NONE: u8 = 0xff;

type Clear = unsafe extern "C" fn(*mut c_void);
unsafe extern "C" {
    fn oracle_canvas_clear_mix_probe(
        candidate: Clear,
        list: *const u8,
        count: usize,
        canvas_mix: u8,
        null_core: bool,
    ) -> u32;
    fn oracle_canvas_clear_mix_original(
        list: *const u8,
        count: usize,
        canvas_mix: u8,
        trace: *mut c_char,
        trace_size: usize,
        rest: *mut c_char,
        rest_size: usize,
    );
    fn oracle_canvas_clear_mix_adapters() -> bool;
    fn obs_canvas_clear_mix_slot(canvas: *mut c_void) -> *mut *mut c_void;
    fn obs_canvas_clear_mix_lock();
    fn obs_canvas_clear_mix_unlock();
    fn obs_canvas_clear_mix_count() -> usize;
    fn obs_canvas_clear_mix_at(index: usize) -> *mut c_void;
    fn obs_canvas_clear_mix_erase(index: usize);
    fn obs_canvas_clear_mix_free(mix: *mut c_void);
}

fn probe(candidate: Clear, list: &[u8], canvas_mix: u8, null_core: bool) -> u32 {
    // SAFETY: the oracle owns the fixture core and canvas every adapter
    // reaches; the list outlives the call; a NULL core is passed only with
    // no canvas mix, where neither side reads the core.
    unsafe {
        oracle_canvas_clear_mix_probe(candidate, list.as_ptr(), list.len(), canvas_mix, null_core)
    }
}

/// The original's trace without the core reads, and the list it left.
fn original(list: &[u8], canvas_mix: u8) -> (String, String) {
    let mut trace = [0 as c_char; 96];
    let mut rest = [0 as c_char; 32];
    // SAFETY: both buffers are writable for their sizes; the oracle writes
    // NUL-terminated text.
    unsafe {
        oracle_canvas_clear_mix_original(
            list.as_ptr(),
            list.len(),
            canvas_mix,
            trace.as_mut_ptr(),
            trace.len(),
            rest.as_mut_ptr(),
            rest.len(),
        );
    }
    // SAFETY: NUL-terminated by the oracle.
    let text = |buffer: &[c_char]| {
        unsafe { CStr::from_ptr(buffer.as_ptr()) }
            .to_str()
            .unwrap()
            .to_owned()
    };
    (text(&trace).replace('o', ""), text(&rest))
}

#[test]
fn real_header_adapters() {
    // SAFETY: the oracle checks the adapters on its own fixture.
    assert!(unsafe { oracle_canvas_clear_mix_adapters() });
}

#[test]
fn original_matches_the_characterization() {
    // No mix: nothing locked, read or freed.
    assert_eq!(original(&[1], NONE), (String::new(), "1".into()));
    // One mix; each position of four.
    assert_eq!(original(&[0], 0), ("LF0U".into(), String::new()));
    for (position, rest) in [(0, "123"), (1, "023"), (2, "013"), (3, "012")] {
        assert_eq!(
            original(&[0, 1, 2, 3], position),
            ("LF".to_owned() + &position.to_string() + "U", rest.into())
        );
    }
    // Unregistered: locked and detached, not freed.
    assert_eq!(original(&[1], 2), ("LU".into(), "1".into()));
    // Duplicate pointer: the first entry only.
    assert_eq!(original(&[5, 5], 5), ("LF5U".into(), "5".into()));
}

const ALPHABET: [u8; 4] = [0, 1, 2, NONE];

fn lists(max: usize) -> Vec<Vec<u8>> {
    let mut all = vec![Vec::new()];
    let mut layer = vec![Vec::new()];
    for _ in 0..max {
        layer = layer
            .iter()
            .flat_map(|list: &Vec<u8>| {
                ALPHABET.iter().map(move |&value| {
                    let mut next = list.clone();
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
    for list in lists(4) {
        for canvas_mix in [NONE, 0, 1, 2, 5] {
            assert_eq!(
                probe(obs_canvas_clear_mix, &list, canvas_mix, false),
                0,
                "{list:?} {canvas_mix}"
            );
        }
        // The early return never reads the core.
        assert_eq!(
            probe(obs_canvas_clear_mix, &list, NONE, true),
            0,
            "{list:?}"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(512), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn any_list_matches_c(list in prop::collection::vec(prop_oneof![9 => 0..8u8, 1 => Just(NONE)], 0..=16),
        canvas_mix in prop_oneof![1 => Just(NONE), 9 => 0..8u8]) {
        prop_assert_eq!(probe(obs_canvas_clear_mix, &list, canvas_mix, false), 0);
    }
}

// ---------------------------------------------------------------------------
// mutants, over the same adapters

fn slot(canvas: *mut c_void) -> *mut *mut c_void {
    // SAFETY: mutants run only on the probe's live canvas.
    unsafe { obs_canvas_clear_mix_slot(canvas) }
}

/// The original's steps, with switches for each broken contract.
#[derive(Clone, Copy, Default)]
struct Breaks {
    lock_without_mix: bool,
    free_before_erase: bool,
    leak: bool,
    detach_before_unlock: bool,
    keep_going: bool,
    count_once: bool,
    keep_unregistered: bool,
    free_after_unlock: bool,
}

/// # Safety
/// The probe's live canvas and fixture core.
unsafe fn run(canvas: *mut c_void, b: Breaks) {
    // SAFETY: the caller's fixture; each adapter is used as the shim does.
    unsafe {
        let mix = *slot(canvas);
        if mix.is_null() && !b.lock_without_mix {
            return;
        }
        obs_canvas_clear_mix_lock();
        let mut found = null_mut();
        let total = if b.count_once {
            obs_canvas_clear_mix_count()
        } else {
            0
        };
        let mut index = 0;
        while index
            < if b.count_once {
                total
            } else {
                obs_canvas_clear_mix_count()
            }
        {
            let entry = obs_canvas_clear_mix_at(index);
            if entry == *slot(canvas) {
                if b.free_before_erase && !b.leak {
                    obs_canvas_clear_mix_free(entry);
                }
                obs_canvas_clear_mix_erase(index);
                found = entry;
                if !b.free_before_erase && !b.leak && !b.free_after_unlock {
                    obs_canvas_clear_mix_free(entry);
                }
                if !b.keep_going {
                    break;
                }
                continue;
            }
            index += 1;
        }
        if b.detach_before_unlock {
            *slot(canvas) = null_mut();
        }
        obs_canvas_clear_mix_unlock();
        if b.free_after_unlock && !found.is_null() {
            obs_canvas_clear_mix_free(found);
        }
        if !(b.keep_unregistered && found.is_null()) {
            *slot(canvas) = null_mut();
        }
    }
}

macro_rules! mutant {
    ($name:ident, $field:ident) => {
        unsafe extern "C" fn $name(canvas: *mut c_void) {
            // SAFETY: called only by the probe with its live canvas.
            unsafe {
                run(
                    canvas,
                    Breaks {
                        $field: true,
                        ..Breaks::default()
                    },
                )
            }
        }
    };
}

unsafe extern "C" fn unbroken(canvas: *mut c_void) {
    // SAFETY: called only by the probe with its live canvas.
    unsafe { run(canvas, Breaks::default()) }
}
mutant!(lock_without_mix, lock_without_mix);
mutant!(free_before_erase, free_before_erase);
mutant!(leak, leak);
mutant!(detach_before_unlock, detach_before_unlock);
mutant!(keep_going, keep_going);
mutant!(count_once, count_once);
mutant!(keep_unregistered, keep_unregistered);
mutant!(free_after_unlock, free_after_unlock);

#[test]
fn mutants_are_caught() {
    let sweep = |candidate: Clear| {
        let mut diff = 0;
        for list in lists(3) {
            for canvas_mix in [NONE, 0, 1, 5] {
                diff |= probe(candidate, &list, canvas_mix, false);
            }
        }
        diff
    };
    // The unbroken model of the steps passes the same sweep.
    assert_eq!(sweep(unbroken), 0);
    for (name, mutant) in [
        ("lock_without_mix", lock_without_mix as Clear),
        ("free_before_erase", free_before_erase),
        ("leak", leak),
        ("detach_before_unlock", detach_before_unlock),
        ("keep_going", keep_going),
        ("count_once", count_once),
        ("keep_unregistered", keep_unregistered),
        ("free_after_unlock", free_after_unlock),
    ] {
        assert_ne!(sweep(mutant), 0, "{name} survived");
    }
}
