//! Tier 3: the original extracted `obs_add_data_path()` and
//! `obs_remove_data_path()` versus the Rust exports, on the production list
//! and adapters of `obs-data-paths-fields.c.h` (oracle/data_paths.c). Each
//! side runs the same adds and removes from an empty list, every added path a
//! fresh copy scribbled over right after the call; the remove answers and the
//! list left (in order, NULL entries marked) must match. Cases: the C
//! tests' sequences, every sequence of up to four steps over NULL, "" and
//! two names (one a prefix of the other), and proptest sequences over more
//! names (case, trailing slash, high bytes).
//!
//! Excluded: another thread's change of the list, outside the original's
//! contract. No intentional differences. Mutants show the probe catches a
//! last-match remove, a case-blind match, a NULL path that matches nothing
//! and an add that drops "".
use core::ffi::{CStr, c_char};
use core::ptr;
use std::ffi::CString;
use std::sync::Mutex;

use obs_core::ffi::data_paths::{obs_add_data_path, obs_remove_data_path};
use proptest::prelude::*;

type Add = unsafe extern "C" fn(*const c_char);
type Remove = unsafe extern "C" fn(*const c_char) -> bool;

unsafe extern "C" {
    fn oracle_data_paths_run(
        add: Option<Add>,
        remove: Option<Remove>,
        ops: *const u8,
        paths: *const *const c_char,
        count: usize,
        results: *mut u8,
        list: *mut c_char,
        size: usize,
    ) -> bool;
    fn obs_data_paths_count() -> usize;
    fn obs_data_paths_entry(index: usize) -> *const c_char;
    fn obs_data_paths_push_copy(path: *const c_char);
    fn obs_data_paths_erase(index: usize);
}

/// The list is process-global: one run at a time.
static LIST: Mutex<()> = Mutex::new(());

/// One step: `true` adds, `false` removes; `None` is NULL.
type Step = (bool, Option<&'static [u8]>);

fn run(add: Option<Add>, remove: Option<Remove>, steps: &[Step]) -> (Vec<u8>, String) {
    let strings: Vec<Option<CString>> = steps
        .iter()
        .map(|(_, path)| path.map(|p| CString::new(p).unwrap()))
        .collect();
    let paths: Vec<*const c_char> = strings
        .iter()
        .map(|s| s.as_ref().map_or(ptr::null(), |s| s.as_ptr()))
        .collect();
    let ops: Vec<u8> = steps.iter().map(|(is_add, _)| u8::from(!is_add)).collect();
    let mut results = vec![0u8; steps.len()];
    let mut list = vec![0 as c_char; 4096];
    let _guard = LIST
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // SAFETY: arrays of `steps.len()` entries and a buffer of its size; the
    // lock keeps other runs off the list.
    let fits = unsafe {
        oracle_data_paths_run(
            add,
            remove,
            ops.as_ptr(),
            paths.as_ptr(),
            steps.len(),
            results.as_mut_ptr(),
            list.as_mut_ptr(),
            list.len(),
        )
    };
    assert!(fits);
    // SAFETY: the oracle terminated it.
    let list = unsafe { CStr::from_ptr(list.as_ptr()) };
    (results, list.to_string_lossy().into_owned())
}

const REAL: (Option<Add>, Option<Remove>) = (Some(obs_add_data_path), Some(obs_remove_data_path));

fn differs(add: Option<Add>, remove: Option<Remove>, steps: &[Step]) -> bool {
    run(add, remove, steps) != run(None, None, steps)
}

fn check(steps: &[Step]) -> (Vec<u8>, String) {
    let rust = run(REAL.0, REAL.1, steps);
    assert_eq!(rust, run(None, None, steps), "{steps:?}");
    rust
}

const A: Option<&[u8]> = Some(b"test_data_paths_a/");
const B: Option<&[u8]> = Some(b"test_data_paths_b/");
const EMPTY: Option<&[u8]> = Some(b"");

#[test]
fn c_test_sequences() {
    // test_core_setup_rules_steps: a, b, a; miss; a twice; b.
    let c: Option<&[u8]> = Some(b"core_setup_rules_c/");
    let (results, list) = check(&[(true, A), (true, B), (true, A), (false, c), (false, A)]);
    assert_eq!(results, [2, 2, 2, 0, 1]);
    assert_eq!(list, "test_data_paths_b/\ntest_data_paths_a/\n");
    // NULL and "" are the same path.
    let (results, list) = check(&[
        (true, None),
        (false, EMPTY),
        (false, None),
        (true, EMPTY),
        (true, A),
        (false, None),
    ]);
    assert_eq!(results, [2, 1, 0, 2, 2, 1]);
    assert_eq!(list, "test_data_paths_a/\n");
    // Exact matching, and the empty entry kept as NULL.
    let (results, list) = check(&[
        (true, Some(b"test_data_paths_Case/")),
        (false, Some(b"test_data_paths_case/")),
        (false, Some(b"test_data_paths_Case")),
        (false, Some(b"test_data_paths_Case//")),
        (true, None),
    ]);
    assert_eq!(results, [2, 0, 0, 0, 2]);
    assert_eq!(list, "test_data_paths_Case/\n<null>\n");
}

#[test]
fn every_short_sequence() {
    const PATHS: [Option<&[u8]>; 4] = [None, Some(b""), Some(b"p/"), Some(b"p/q")];
    let mut sequence: Vec<Step> = Vec::new();
    fn walk(sequence: &mut Vec<Step>, depth: usize) {
        check(sequence);
        if depth == 4 {
            return;
        }
        for is_add in [true, false] {
            for path in PATHS {
                sequence.push((is_add, path));
                walk(sequence, depth + 1);
                sequence.pop();
            }
        }
    }
    walk(&mut sequence, 0);
}

unsafe extern "C" fn last_match(path: *const c_char) -> bool {
    let path = if path.is_null() {
        &[][..]
    } else {
        // SAFETY: the oracle's path.
        unsafe { CStr::from_ptr(path) }.to_bytes()
    };
    // SAFETY: the oracle's list, under the run lock.
    unsafe {
        for index in (0..obs_data_paths_count()).rev() {
            let entry = obs_data_paths_entry(index);
            let entry = if entry.is_null() {
                &[][..]
            } else {
                CStr::from_ptr(entry).to_bytes()
            };
            if entry == path {
                obs_data_paths_erase(index);
                return true;
            }
        }
    }
    false
}
unsafe extern "C" fn case_blind(path: *const c_char) -> bool {
    if path.is_null() {
        // SAFETY: NULL is the real export's.
        return unsafe { obs_remove_data_path(path) };
    }
    // SAFETY: the oracle's path and list, under the run lock.
    unsafe {
        let path = CStr::from_ptr(path).to_bytes();
        for index in 0..obs_data_paths_count() {
            let entry = obs_data_paths_entry(index);
            if !entry.is_null() && CStr::from_ptr(entry).to_bytes().eq_ignore_ascii_case(path) {
                obs_data_paths_erase(index);
                return true;
            }
        }
    }
    false
}
unsafe extern "C" fn null_misses(path: *const c_char) -> bool {
    // SAFETY: the real export, for a non-NULL path.
    !path.is_null() && unsafe { obs_remove_data_path(path) }
}
unsafe extern "C" fn drops_empty(path: *const c_char) {
    // SAFETY: the oracle's path (or NULL).
    if path.is_null() || unsafe { *path } != 0 {
        // SAFETY: as above.
        unsafe { obs_data_paths_push_copy(path) }
    }
}

#[test]
fn mutants_are_caught() {
    let lower: Option<&[u8]> = Some(b"test_data_paths_a/");
    let upper: Option<&[u8]> = Some(b"TEST_DATA_PATHS_A/");
    assert!(differs(
        REAL.0,
        Some(last_match),
        &[(true, A), (true, B), (true, A), (false, A)]
    ));
    assert!(differs(
        REAL.0,
        Some(case_blind),
        &[(true, upper), (false, lower)]
    ));
    assert!(differs(
        REAL.0,
        Some(null_misses),
        &[(true, EMPTY), (false, None)]
    ));
    assert!(differs(Some(drops_empty), REAL.1, &[(true, EMPTY)]));
    assert!(!differs(
        REAL.0,
        REAL.1,
        &[(true, EMPTY), (true, A), (false, None)]
    ));
}

proptest! {
    #[test]
    fn any_sequence(steps in proptest::collection::vec((any::<bool>(), 0usize..7), 0..20)) {
        const PATHS: [Option<&[u8]>; 7] = [
            None,
            Some(b""),
            Some(b"a/"),
            Some(b"A/"),
            Some(b"a"),
            Some(b"a//"),
            Some(b"\xffa/"),
        ];
        let steps: Vec<Step> = steps.into_iter().map(|(is_add, path)| (is_add, PATHS[path])).collect();
        check(&steps);
    }
}
