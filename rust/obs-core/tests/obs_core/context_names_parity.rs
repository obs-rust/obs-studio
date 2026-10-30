//! Tier 3: the shims against the original `libobs/obs-context-names.c.h`.
//!
//! The oracle (`rust/obs-c-oracle/oracle/context_names.c`) is the C file,
//! unmodified, with its two functions renamed. Both are called through
//! their C signatures with the same scripted `exists()` (a set of taken
//! names, or "taken for the first n questions", and whether NULL is taken),
//! which logs every question. Compared, for each call:
//!
//! - the result: NULL or not, and the bytes;
//! - the full log of `exists()` calls: how many, in what order, each name
//!   byte for byte (NULL included), each through the caller's `param`, and
//!   that the first one, and only the first, got the caller's own `name`
//!   pointer;
//! - the counter after `obs_context_name_dup`;
//! - that each result is a `bmalloc` block apart from the name, which
//!   `bfree` releases. (Leaks are `test_context_name_helpers`'s: the
//!   allocation count is global to this test binary, whose tests run in
//!   parallel.)
//!
//! Exhaustive over small cases: NULL, empty, ASCII, `%` sequences, bytes
//! that are not UTF-8, names ending in digits or `" N"`, `"(null)"` and
//! unnamed names, each against taken sets of the name and its candidates
//! (runs, gaps, `" 0"` and `" 1"`), "the first n questions taken" for n up
//! to 12, and NULL taken or not; the unnamed name, private on and off, at
//! counter values near 0, near every width of `%04lld` and at both ends of
//! `long long`. Proptests (4096 cases each) over random names and taken
//! sets.
//!
//! Excluded, the intentional differences, tested on the shims alone: a null
//! `exists` and a null `unnamed_index` where an unnamed name is needed (C
//! calls through and dereferences them). `test_context_name_helpers`
//! compiles the C into itself, so this is what compares the swap;
//! `test_context_names` runs it through the real libobs.

use std::collections::HashSet;
use std::ffi::{CStr, CString, c_char, c_longlong, c_void};
use std::ptr;

use obs_c_oracle::context_names::{
    OracleContextNameExists, oracle_obs_context_name_deduplicate, oracle_obs_context_name_dup,
};
use obs_core::ffi::context_names::{
    obs_context_name_deduplicate, obs_context_name_dup, obs_context_name_exists_t,
};
use proptest::prelude::*;

unsafe extern "C" {
    fn bfree(ptr: *mut c_void);
}

/// The scripted `exists()`: which names are taken.
#[derive(Debug, Clone, Default)]
struct Script {
    taken: HashSet<Vec<u8>>,
    /// The first `always_taken` questions are taken, whatever the name.
    always_taken: usize,
    /// What a NULL name gets.
    null_taken: bool,
}

/// One call of `exists()`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Call {
    name: Option<Vec<u8>>,
    /// It got the caller's own `name` pointer.
    callers_pointer: bool,
}

struct State {
    script: Script,
    callers_name: *const c_char,
    calls: Vec<Call>,
}

unsafe extern "C" fn exists(param: *mut c_void, name: *const c_char) -> bool {
    // SAFETY: every caller here passes a `State` as `param`.
    let state = unsafe { &mut *param.cast::<State>() };
    let bytes = (!name.is_null())
        // SAFETY: the shims and the C pass NUL-terminated names.
        .then(|| unsafe { CStr::from_ptr(name) }.to_bytes().to_vec());
    let n = state.calls.len();
    let taken = match &bytes {
        None => state.script.null_taken,
        Some(b) => n < state.script.always_taken || state.script.taken.contains(b),
    };
    state.calls.push(Call {
        name: bytes,
        callers_pointer: ptr::eq(name, state.callers_name),
    });
    taken
}

/// What one implementation did.
#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    result: Option<Vec<u8>>,
    calls: Vec<Call>,
    counter: i64,
}

/// Takes a returned name: its bytes, then `bfree`.
fn take(result: *mut c_char) -> Option<Vec<u8>> {
    if result.is_null() {
        return None;
    }
    // SAFETY: a returned name is NUL-terminated and `bmalloc`'d.
    let bytes = unsafe { CStr::from_ptr(result) }.to_bytes().to_vec();
    // SAFETY: as above; freed once.
    unsafe { bfree(result.cast()) };
    Some(bytes)
}

type Dedup =
    unsafe extern "C" fn(*const c_char, OracleContextNameExists, *mut c_void) -> *mut c_char;
type Dup = unsafe extern "C" fn(*const c_char, bool, *mut c_longlong) -> *mut c_char;

const SHIM_DEDUP: Dedup = obs_context_name_deduplicate;
const C_DEDUP: Dedup = oracle_obs_context_name_deduplicate;
const SHIM_DUP: Dup = obs_context_name_dup;
const C_DUP: Dup = oracle_obs_context_name_dup;

fn run_dedup(f: Dedup, name: Option<&CStr>, script: &Script) -> Outcome {
    let name_ptr = name.map_or(ptr::null(), CStr::as_ptr);
    let mut state = State {
        script: script.clone(),
        callers_name: name_ptr,
        calls: Vec::new(),
    };
    let exists_fn: obs_context_name_exists_t = Some(exists);
    // SAFETY: a NUL-terminated name or NULL, a valid callback and its state.
    let raw = unsafe { f(name_ptr, exists_fn, (&raw mut state).cast()) };
    let result = take(raw);
    Outcome {
        result,
        calls: state.calls,
        counter: 0,
    }
}

fn run_dup(f: Dup, name: Option<&CStr>, private: bool, counter: i64) -> Outcome {
    let name_ptr = name.map_or(ptr::null(), CStr::as_ptr);
    let mut counter = counter;
    // SAFETY: a NUL-terminated name or NULL, and a valid counter.
    let raw = unsafe { f(name_ptr, private, &raw mut counter) };
    if !raw.is_null() {
        assert!(!ptr::eq(raw, name_ptr), "a copy, not the name");
    }
    let result = take(raw);
    Outcome {
        result,
        calls: Vec::new(),
        counter,
    }
}

fn check_dedup(name: Option<&CStr>, script: &Script) {
    let c = run_dedup(C_DEDUP, name, script);
    let rust = run_dedup(SHIM_DEDUP, name, script);
    assert_eq!(rust, c, "name {name:?}, script {script:?}");
    // The C asks the caller's own pointer first, and only first.
    assert!(c.calls.first().is_some_and(|call| call.callers_pointer));
    assert!(c.calls.iter().skip(1).all(|call| !call.callers_pointer));
}

fn check_dup(name: Option<&CStr>, private: bool, counter: i64) {
    let c = run_dup(C_DUP, name, private, counter);
    let rust = run_dup(SHIM_DUP, name, private, counter);
    assert_eq!(
        rust, c,
        "name {name:?}, private {private}, counter {counter}"
    );
}

/// The small names of the exhaustive sweep (NULL is added separately).
fn names() -> Vec<Vec<u8>> {
    let mut names: Vec<Vec<u8>> = [
        &b""[..],
        b" ",
        b"  ",
        b"A",
        b"a",
        b"A ",
        b" A",
        b"A 2",
        b"A 02",
        b"A 1",
        b"A 0",
        b"Cam 2",
        b"Cam 2 2",
        b"X (2)",
        b"Y2",
        b"9",
        b"Z 0",
        b"N -1",
        b"Big 2147483647",
        b"Big 2147483648",
        b"%s %d %n %%",
        b"%s",
        b"%d",
        b"%n",
        b"\xff\xfe\x80",
        b"\x80",
        b"\t\n",
        b"\r",
        "\u{e9}".as_bytes(),
        "e\u{301}".as_bytes(),
        b"__unnamed0000",
        b"__unnamed0002 2",
        b"(null)",
        b"(null) 2",
        b"Main",
    ]
    .iter()
    .map(|n| n.to_vec())
    .collect();
    names.push(vec![b'L'; 64 * 1024]);
    names
}

fn with_suffix(name: &[u8], n: i64) -> Vec<u8> {
    [name, format!(" {n}").as_bytes()].concat()
}

/// Taken sets for `name`: none, the name, runs of candidates, gaps, the
/// never-tried `" 0"` and `" 1"`, candidates without the name, and other
/// names.
fn scripts(name: &[u8]) -> Vec<Script> {
    let set = |names: Vec<Vec<u8>>| Script {
        taken: names.into_iter().collect(),
        ..Script::default()
    };
    let mut scripts = vec![
        Script::default(),
        set(vec![name.to_vec()]),
        set(vec![name.to_vec(), with_suffix(name, 3)]),
        set(vec![
            name.to_vec(),
            with_suffix(name, 2),
            with_suffix(name, 4),
        ]),
        set(vec![
            name.to_vec(),
            with_suffix(name, 0),
            with_suffix(name, 1),
        ]),
        set(vec![with_suffix(name, 2), with_suffix(name, 3)]),
        set(vec![
            name.to_vec(),
            b"A".to_vec(),
            b"A 2".to_vec(),
            b"".to_vec(),
        ]),
        set(vec![name.to_vec(), with_suffix(&with_suffix(name, 2), 2)]),
    ];
    for run in 1..=8 {
        let mut taken = vec![name.to_vec()];
        taken.extend((2..2 + run).map(|n| with_suffix(name, n)));
        scripts.push(set(taken));
    }
    for n in 0..=12 {
        scripts.push(Script {
            always_taken: n,
            ..Script::default()
        });
    }
    scripts
}

#[test]
fn dedup_matches_the_c_over_small_cases() {
    for name in names() {
        let c_name = CString::new(name.clone()).expect("no NUL");
        for script in scripts(&name) {
            for null_taken in [false, true] {
                let script = Script {
                    null_taken,
                    ..script.clone()
                };
                check_dedup(Some(&c_name), &script);
            }
        }
    }
}

/// NULL goes to `exists()` as it is; when it is called taken, both print
/// `"(null) 2"`, ... .
#[test]
fn dedup_matches_the_c_for_a_null_name() {
    let null = b"(null)";
    for script in scripts(null) {
        for null_taken in [false, true] {
            let script = Script {
                null_taken,
                ..script.clone()
            };
            check_dedup(None, &script);
        }
    }
}

/// Counter values near 0, near each width of `%04lld` with either sign,
/// and at both ends of `long long` (`LLONG_MAX` wraps).
fn counters() -> Vec<i64> {
    let mut counters = vec![i64::MIN, i64::MIN + 1, i64::MAX - 1, i64::MAX];
    let mut edge: i64 = 1;
    for _ in 0..19 {
        for v in [edge - 1, edge, edge + 1] {
            counters.extend([v, -v]);
        }
        edge = edge.saturating_mul(10);
    }
    counters.extend([
        i64::from(i32::MAX),
        i64::from(i32::MAX) + 1,
        i64::from(i32::MIN),
        i64::from(i32::MIN) - 1,
        i64::from(u32::MAX),
    ]);
    counters.sort_unstable();
    counters.dedup();
    counters
}

#[test]
fn dup_matches_the_c_over_small_cases() {
    let names = names();
    for counter in counters() {
        for private in [false, true] {
            check_dup(None, private, counter);
            for name in &names {
                let c_name = CString::new(name.clone()).expect("no NUL");
                check_dup(Some(&c_name), private, counter);
            }
        }
    }
}

/// The counter in sequence, as libobs uses it: the same counter through a
/// run of calls.
#[test]
fn dup_sequence_matches_the_c() {
    let names: [Option<&CStr>; 6] = [None, Some(c""), Some(c"C"), None, Some(c""), Some(c" ")];
    for start in [0, 9998, -2, i64::MAX - 2] {
        let mut c_counter = start;
        let mut rust_counter = start;
        for (i, name) in names.iter().cycle().take(24).enumerate() {
            let private = i % 4 == 3;
            let c = run_dup(C_DUP, *name, private, c_counter);
            let rust = run_dup(SHIM_DUP, *name, private, rust_counter);
            assert_eq!(rust, c, "step {i} from {start}");
            c_counter = c.counter;
            rust_counter = rust.counter;
        }
    }
}

// ---------------------------------------------------------------------------
// The intentional differences, on the shims alone

/// A null `exists` returns NULL without a call (C calls it).
#[test]
fn null_exists_returns_null() {
    // SAFETY: a NUL-terminated name; the callback is null.
    let result = unsafe { obs_context_name_deduplicate(c"A".as_ptr(), None, ptr::null_mut()) };
    assert!(result.is_null());
}

/// A null `unnamed_index` returns NULL where an unnamed name is needed (C
/// dereferences it); a name, or a private NULL name, does not need it.
#[test]
fn null_counter_only_where_needed() {
    // SAFETY: names are NUL-terminated or NULL; the counter is null.
    unsafe {
        assert!(obs_context_name_dup(ptr::null(), false, ptr::null_mut()).is_null());
        assert!(obs_context_name_dup(c"".as_ptr(), false, ptr::null_mut()).is_null());
        assert!(obs_context_name_dup(c"".as_ptr(), true, ptr::null_mut()).is_null());
        assert!(obs_context_name_dup(ptr::null(), true, ptr::null_mut()).is_null());
        let named = obs_context_name_dup(c"A".as_ptr(), false, ptr::null_mut());
        assert_eq!(take(named).as_deref(), Some(&b"A"[..]));
    }
}

// ---------------------------------------------------------------------------
// Proptests

fn name_bytes() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(
        prop_oneof![
            3 => (1u8..=255).prop_map(|b| vec![b]),
            2 => prop::sample::select(vec![
                b" 2".to_vec(),
                b" 3".to_vec(),
                b" (2)".to_vec(),
                b" ".to_vec(),
                b"%s".to_vec(),
                b"%n".to_vec(),
                b"\xff".to_vec(),
                "\u{e9}".as_bytes().to_vec(),
                b"__unnamed".to_vec(),
                b"0".to_vec(),
                b"9".to_vec(),
                b"-1".to_vec(),
            ]),
        ],
        0..8,
    )
    .prop_map(|parts| parts.concat())
}

prop_compose! {
    fn script_for(name: Vec<u8>)(
        suffixes in prop::collection::vec(0i64..16, 0..12),
        others in prop::collection::vec(name_bytes(), 0..4),
        name_taken in any::<bool>(),
        always_taken in prop_oneof![4 => Just(0usize), 1 => 0usize..10],
        null_taken in any::<bool>(),
    ) -> Script {
        let mut taken: HashSet<Vec<u8>> = others.into_iter().collect();
        taken.extend(suffixes.into_iter().map(|n| with_suffix(&name, n)));
        if name_taken {
            taken.insert(name.clone());
        }
        Script { taken, always_taken, null_taken }
    }
}

fn case() -> impl Strategy<Value = (Option<Vec<u8>>, Script)> {
    prop::option::weighted(0.95, name_bytes()).prop_flat_map(|name| {
        let base = name.clone().unwrap_or_else(|| b"(null)".to_vec());
        (Just(name), script_for(base))
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(4096)))]

    #[test]
    fn dedup_matches_the_c_for_random_names((name, script) in case()) {
        let c_name = name.map(|n| CString::new(n).expect("no NUL"));
        check_dedup(c_name.as_deref(), &script);
    }

    #[test]
    fn dup_matches_the_c_for_random_names(
        name in prop::option::of(name_bytes()),
        private in any::<bool>(),
        counter in prop_oneof![any::<i64>(), -20_000i64..20_000],
    ) {
        let c_name = name.map(|n| CString::new(n).expect("no NUL"));
        check_dup(c_name.as_deref(), private, counter);
    }
}
