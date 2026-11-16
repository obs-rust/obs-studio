//! Tier 3: unchanged original C with real hash/layout, probing public setter calls.
//! Initialized core and non-aliasing nullable strings are the valid C domain.
use obs_hotkey::ffi::pair::obs_hotkey_pair;
use obs_hotkey::ffi::pair_meta::{obs_hotkey_pair_set_descriptions, obs_hotkey_pair_set_names};
use proptest::prelude::*;
use std::cell::Cell;
use std::ffi::{CString, c_char};

type Entry = unsafe extern "C" fn(usize, *const c_char, *const c_char);
unsafe extern "C" {
    fn oracle_obs_hotkey_pair_set_names(id: usize, first: *const c_char, second: *const c_char);
    fn oracle_obs_hotkey_pair_set_descriptions(
        id: usize,
        first: *const c_char,
        second: *const c_char,
    );
    fn oracle_pair_meta_reset(
        id: usize,
        present: bool,
        redirect: bool,
        first: *const c_char,
        second: *const c_char,
    );
    fn oracle_pair_meta_find(id: usize) -> *mut obs_hotkey_pair;
    fn oracle_pair_meta_id(pair: *const obs_hotkey_pair, half: usize) -> usize;
    fn oracle_pair_meta_finish(out: *mut usize) -> usize;
}
thread_local! { static ACTIVE: Cell<bool> = const { Cell::new(false) }; }

pub(super) fn find(id: usize) -> Option<*mut obs_hotkey_pair> {
    ACTIVE.with(|active| {
        active.get().then(|| {
            // SAFETY: this thread installed the initialized oracle registry.
            unsafe { oracle_pair_meta_find(id) }
        })
    })
}
pub(super) fn half_id(pair: *const obs_hotkey_pair, half: usize) -> Option<usize> {
    ACTIVE.with(|active| {
        active.get().then(|| {
            // SAFETY: the production shim passes only a live token and half 0 or 1.
            unsafe { oracle_pair_meta_id(pair, half) }
        })
    })
}
fn run(
    entry: Entry,
    registered: usize,
    queried: usize,
    present: bool,
    redirect: bool,
    values: [*const c_char; 2],
) -> Vec<usize> {
    let mut trace = [0; 32];
    ACTIVE.with(|active| active.set(true));
    // SAFETY: inputs live through the synchronous call; output has room for all
    // probe calls. Reset/finish own and free the real thread-local uthash table.
    let count = unsafe {
        oracle_pair_meta_reset(registered, present, redirect, values[0], values[1]);
        entry(queried, values[0], values[1]);
        oracle_pair_meta_finish(trace.as_mut_ptr())
    };
    ACTIVE.with(|active| active.set(false));
    trace[..count].to_vec()
}
fn compare(
    registered: usize,
    queried: usize,
    present: bool,
    redirect: bool,
    first: Option<&[u8]>,
    second: Option<&[u8]>,
    shared: bool,
) {
    let first = first.map(|v| CString::new(v).unwrap());
    let second = second.map(|v| CString::new(v).unwrap());
    let ptr = |v: &Option<CString>| v.as_ref().map_or(std::ptr::null(), |s| s.as_ptr());
    let values = [ptr(&first), if shared { ptr(&first) } else { ptr(&second) }];
    for (rust, c, field) in [
        (
            obs_hotkey_pair_set_names as Entry,
            oracle_obs_hotkey_pair_set_names as Entry,
            0,
        ),
        (
            obs_hotkey_pair_set_descriptions as Entry,
            oracle_obs_hotkey_pair_set_descriptions as Entry,
            1,
        ),
    ] {
        let expected = run(c, registered, queried, present, redirect, values);
        let actual = run(rust, registered, queried, present, redirect, values);
        assert_eq!(actual, expected);
        if present && queried == registered {
            assert_eq!(
                actual,
                [
                    field,
                    11,
                    0,
                    field,
                    if redirect { 33 } else { 22 },
                    usize::from(values[0] != values[1])
                ]
            );
        } else {
            assert!(actual.is_empty());
        }
    }
}
#[test]
fn characterized_ids_null_empty_raw_shared_and_live_second_id() {
    for id in [0, 1, 99, usize::MAX, usize::MAX - 1] {
        for present in [false, true] {
            for redirect in [false, true] {
                for (first, second) in [
                    (None, None),
                    (None, Some(b"".as_slice())),
                    (Some(b"raw\xff\x80\t".as_slice()), None),
                    (Some(b"name".as_slice()), Some(b"description".as_slice())),
                ] {
                    for query in [id, id ^ 1] {
                        for shared in [false, true] {
                            compare(id, query, present, redirect, first, second, shared);
                        }
                    }
                }
            }
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(128)))]
    #[test]
    fn arbitrary_ids_and_nullable_byte_strings(
        id in any::<usize>(), known in any::<bool>(), present in any::<bool>(), redirect in any::<bool>(), shared in any::<bool>(),
        first in proptest::option::of(proptest::collection::vec(1u8..=255, 0..32)),
        second in proptest::option::of(proptest::collection::vec(1u8..=255, 0..32)),
    ) { compare(id, if known { id } else { id ^ 1 }, present, redirect, first.as_deref(), second.as_deref(), shared); }
}
