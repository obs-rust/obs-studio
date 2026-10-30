//! Tier 3 parity: the shims against the original
//! `libobs/obs-nix-portal-shortcut-sets.c.h` (oracle/portal_shortcut_sets.c).
//!
//! `test_portal_shortcut_sets` and `test_portal_shortcuts` compile the C
//! into themselves, so they never see the Rust swap (a variant of the first
//! linked against `libobs_rust-static` does not link on its own: see
//! `test/cmocka/tests.d/test_portal_shortcut_sets.cmake`). This file is
//! what compares the two.
//!
//! Both sides get the same arrays through the same views. Exhaustively over
//! small sets, then over random ones:
//! - the combinations come from a pool with `OBS_KEY_NONE`, a mouse button,
//!   the same key with other modifiers and with stray modifier bits (Caps
//!   Lock, bit 31), and every sequence of them up to a length, duplicates
//!   included;
//! - the descriptions are NULL, empty, the same bytes at another address,
//!   other bytes, the same name in NFC and NFD, case, a prefix and bytes
//!   that are not UTF-8, with fewer or more entries than combinations;
//! - every combination of the session, retry and partly-declined flags.
//!
//! After each call this compares the return value, the counts, and every
//! entry of the position buffers: they start as a sentinel and have two
//! more entries than current, so the entries past the counts (the
//! partition's first pass leaves approved positions there) and any write
//! past current's size are compared too. Empty views with a NULL array,
//! and NULL position buffers for an empty current, are defined in C and
//! compared as well.
//!
//! Intentional differences, excluded here and checked on the Rust side
//! only: a null view, a view with a null array and a nonzero count, a null
//! count out-pointer and a null position buffer that C would write to are
//! undefined in C (it dereferences them); Rust reads an empty set and skips
//! the write. Nothing else is excluded.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use std::ffi::{CString, c_char};

use obs_c_oracle::portal_shortcut_sets::{
    LAYOUT_NAMES, OracleCombos, OracleDescriptions, OracleKeyCombination,
    oracle_obs_nix_portal_sets_has, oracle_obs_nix_portal_sets_partition,
    oracle_obs_nix_portal_sets_request, oracle_obs_nix_portal_sets_unchanged,
    oracle_portal_sets_layout,
};
use obs_hotkey::ffi::nix_hotkey_state::obs_key_combination;
use obs_hotkey::ffi::portal_shortcut_sets::{
    obs_nix_portal_combos, obs_nix_portal_descriptions, obs_nix_portal_sets_has,
    obs_nix_portal_sets_partition, obs_nix_portal_sets_request, obs_nix_portal_sets_unchanged,
};
use obs_hotkey::name_map::{KEY_NONE, OBS_KEY_MOUSE1};
use obs_hotkey::nix_hotkey_state::{INTERACT_CONTROL_KEY, INTERACT_SHIFT_KEY};
use proptest::prelude::*;

const CAPS: u32 = 1;
const KEY_A: i32 = 0x41;
const SENTINEL: usize = 0x5e47_1ae1;

fn kc(modifiers: u32, key: i32) -> obs_key_combination {
    obs_key_combination { modifiers, key }
}

/// The exhaustive pool: a key, the same key with Ctrl, with a stray bit
/// and with every bit, `OBS_KEY_NONE` (with and without a modifier) and a
/// mouse button.
fn pool() -> Vec<obs_key_combination> {
    vec![
        kc(0, KEY_A),
        kc(INTERACT_CONTROL_KEY, KEY_A),
        kc(CAPS | INTERACT_SHIFT_KEY, KEY_A),
        kc(0, KEY_NONE),
        kc(INTERACT_CONTROL_KEY, KEY_NONE),
        kc(1 << 31, OBS_KEY_MOUSE1),
    ]
}

/// Every sequence of `pool` entries of length `0..=max`.
fn sequences<T: Clone>(pool: &[T], max: usize) -> Vec<Vec<T>> {
    let mut all = vec![Vec::new()];
    let mut last = vec![Vec::new()];
    for _ in 0..max {
        let mut next = Vec::new();
        for s in &last {
            for p in pool {
                let mut t = s.clone();
                t.push(p.clone());
                next.push(t);
            }
        }
        all.extend(next.iter().cloned());
        last = next;
    }
    all
}

/// The description pool: owned strings, so equal bytes sit at different
/// addresses. `None` is a NULL pointer.
struct Strings(Vec<CString>);

impl Strings {
    fn new() -> Self {
        let bytes: [&[u8]; 11] = [
            b"",
            b"x",
            b"x",
            b"y",
            b"OBS: Mic \xc3\xa9",
            b"OBS: Mic e\xcc\x81",
            b"OBS: MIC",
            b"OBS: mic",
            b"OBS: Mic",
            b"\xff\xfe\x80 not UTF-8",
            b"\xff\xfe\x80 not UTF-8",
        ];
        Self(bytes.iter().map(|b| CString::new(*b).unwrap()).collect())
    }

    /// The pointers for the given picks (`None`: NULL).
    fn ptrs(&self, picks: &[Option<usize>]) -> Vec<*const c_char> {
        picks
            .iter()
            .map(|p| p.map_or(std::ptr::null(), |i| self.0[i].as_ptr()))
            .collect()
    }
}

/// Both views of one combination array. A NULL array when it is empty
/// and `null_when_empty`, as `da_init` leaves a darray.
struct Combos {
    items: Vec<obs_key_combination>,
    null_when_empty: bool,
}

impl Combos {
    fn new(items: &[obs_key_combination], null_when_empty: bool) -> Self {
        Self {
            items: items.to_vec(),
            null_when_empty,
        }
    }
    fn ptr(&self) -> *const obs_key_combination {
        if self.items.is_empty() && self.null_when_empty {
            std::ptr::null()
        } else {
            self.items.as_ptr()
        }
    }
    fn rust(&self) -> obs_nix_portal_combos {
        obs_nix_portal_combos {
            array: self.ptr(),
            num: self.items.len(),
        }
    }
    fn c(&self) -> OracleCombos {
        OracleCombos {
            array: self.ptr().cast::<OracleKeyCombination>(),
            num: self.items.len(),
        }
    }
}

fn desc_rust(p: &[*const c_char], null_when_empty: bool) -> obs_nix_portal_descriptions {
    obs_nix_portal_descriptions {
        array: if p.is_empty() && null_when_empty {
            std::ptr::null()
        } else {
            p.as_ptr()
        },
        num: p.len(),
    }
}

fn desc_c(p: &[*const c_char], null_when_empty: bool) -> OracleDescriptions {
    OracleDescriptions {
        array: if p.is_empty() && null_when_empty {
            std::ptr::null()
        } else {
            p.as_ptr()
        },
        num: p.len(),
    }
}

fn oracle_combo(c: obs_key_combination) -> OracleKeyCombination {
    OracleKeyCombination {
        modifiers: c.modifiers,
        key: c.key,
    }
}

fn check_has(set: &[obs_key_combination], combo: obs_key_combination, null: bool) {
    let set = Combos::new(set, null);
    let (r, c) = (set.rust(), set.c());
    // SAFETY: valid views of live arrays.
    let rust = unsafe { obs_nix_portal_sets_has(&raw const r, combo) };
    // SAFETY: as above.
    let want = unsafe { oracle_obs_nix_portal_sets_has(&raw const c, oracle_combo(combo)) };
    assert_eq!(rust, want, "has {:?} in {:?}", combo, set.items);
}

#[allow(clippy::too_many_arguments)]
fn check_unchanged(
    flags: u8,
    current: &[obs_key_combination],
    current_desc: &[*const c_char],
    bound: &[obs_key_combination],
    bound_desc: &[*const c_char],
    declined: &[obs_key_combination],
    null: bool,
) {
    let (session, retry, partly) = (flags & 1 != 0, flags & 2 != 0, flags & 4 != 0);
    let (cur, bnd, dec) = (
        Combos::new(current, null),
        Combos::new(bound, null),
        Combos::new(declined, null),
    );
    let (cr, br, dr) = (cur.rust(), bnd.rust(), dec.rust());
    let (cc, bc, dc) = (cur.c(), bnd.c(), dec.c());
    let (cdr, bdr) = (desc_rust(current_desc, null), desc_rust(bound_desc, null));
    let (cdc, bdc) = (desc_c(current_desc, null), desc_c(bound_desc, null));
    // SAFETY: valid views of live arrays and NUL-terminated strings.
    let rust = unsafe {
        obs_nix_portal_sets_unchanged(
            session,
            retry,
            partly,
            &raw const cr,
            &raw const cdr,
            &raw const br,
            &raw const bdr,
            &raw const dr,
        )
    };
    // SAFETY: as above.
    let want = unsafe {
        oracle_obs_nix_portal_sets_unchanged(
            session,
            retry,
            partly,
            &raw const cc,
            &raw const cdc,
            &raw const bc,
            &raw const bdc,
            &raw const dc,
        )
    };
    assert_eq!(
        rust, want,
        "unchanged flags {flags:03b} current {current:?} {current_desc:?} bound {bound:?} {bound_desc:?} declined {declined:?}"
    );
}

/// What a request or partition wrote: the return value, the counts and the
/// whole buffers.
#[derive(Debug, PartialEq, Eq)]
struct Written {
    result: bool,
    counts: Vec<usize>,
    buffers: Vec<Vec<usize>>,
}

fn check_request(
    current: &[obs_key_combination],
    partly: bool,
    declined: &[obs_key_combination],
    null: bool,
) {
    let (cur, dec) = (Combos::new(current, null), Combos::new(declined, null));
    let (cr, dr, cc, dc) = (cur.rust(), dec.rust(), cur.c(), dec.c());
    let room = current.len() + 2;
    let run = |rust: bool| {
        let mut out = vec![SENTINEL; room];
        let mut num = SENTINEL;
        // A NULL buffer is defined when nothing is written to it.
        let buf = if current.is_empty() && null {
            std::ptr::null_mut()
        } else {
            out.as_mut_ptr()
        };
        // SAFETY: valid views; room for current's size and more.
        let result = unsafe {
            if rust {
                obs_nix_portal_sets_request(&raw const cr, partly, &raw const dr, buf, &raw mut num)
            } else {
                oracle_obs_nix_portal_sets_request(
                    &raw const cc,
                    partly,
                    &raw const dc,
                    buf,
                    &raw mut num,
                )
            }
        };
        Written {
            result,
            counts: vec![num],
            buffers: vec![out],
        }
    };
    assert_eq!(
        run(true),
        run(false),
        "request {current:?} partly {partly} declined {declined:?}"
    );
}

fn check_partition(
    current: &[obs_key_combination],
    approved: &[obs_key_combination],
    request: &[obs_key_combination],
    null: bool,
) {
    let (cur, app, req) = (
        Combos::new(current, null),
        Combos::new(approved, null),
        Combos::new(request, null),
    );
    let (cr, ar, rr, cc, ac, rc) = (
        cur.rust(),
        app.rust(),
        req.rust(),
        cur.c(),
        app.c(),
        req.c(),
    );
    let room = current.len() + 2;
    let run = |rust: bool| {
        let mut part = vec![SENTINEL; room];
        let mut declined = vec![SENTINEL; room];
        let (mut num_part, mut num_declined) = (SENTINEL, SENTINEL);
        let (p, d) = if current.is_empty() && null {
            (std::ptr::null_mut(), std::ptr::null_mut())
        } else {
            (part.as_mut_ptr(), declined.as_mut_ptr())
        };
        // SAFETY: valid views; two separate buffers with room for current.
        let result = unsafe {
            if rust {
                obs_nix_portal_sets_partition(
                    &raw const cr,
                    &raw const ar,
                    &raw const rr,
                    p,
                    &raw mut num_part,
                    d,
                    &raw mut num_declined,
                )
            } else {
                oracle_obs_nix_portal_sets_partition(
                    &raw const cc,
                    &raw const ac,
                    &raw const rc,
                    p,
                    &raw mut num_part,
                    d,
                    &raw mut num_declined,
                )
            }
        };
        Written {
            result,
            counts: vec![num_part, num_declined],
            buffers: vec![part, declined],
        }
    };
    assert_eq!(
        run(true),
        run(false),
        "partition {current:?} approved {approved:?} request {request:?}"
    );
}

#[test]
fn layouts_match_c() {
    use std::mem::{align_of, offset_of, size_of};
    let rust = [
        size_of::<obs_nix_portal_combos>(),
        align_of::<obs_nix_portal_combos>(),
        offset_of!(obs_nix_portal_combos, array),
        offset_of!(obs_nix_portal_combos, num),
        size_of::<obs_nix_portal_descriptions>(),
        align_of::<obs_nix_portal_descriptions>(),
        offset_of!(obs_nix_portal_descriptions, array),
        offset_of!(obs_nix_portal_descriptions, num),
        size_of::<usize>(),
        size_of::<bool>(),
    ];
    for (which, (name, value)) in LAYOUT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain lookup.
        assert_eq!(value, unsafe { oracle_portal_sets_layout(which) }, "{name}");
    }
    // SAFETY: as above.
    assert_eq!(unsafe { oracle_portal_sets_layout(LAYOUT_NAMES.len()) }, 0);
    // A description entry is a `const char *`, NULL included.
    assert_eq!(
        size_of::<Option<obs_hotkey::ffi::portal_shortcut_sets::Description>>(),
        size_of::<*const c_char>()
    );
}

#[test]
fn has_every_small_set_matches_c() {
    let pool = pool();
    let mut probes = pool.clone();
    probes.push(kc(INTERACT_SHIFT_KEY, KEY_A));
    probes.push(kc(u32::MAX, i32::MIN));
    for set in sequences(&pool, 3) {
        for &probe in &probes {
            check_has(&set, probe, false);
            check_has(&set, probe, true);
        }
    }
}

/// With a session: every pair of sets up to two entries, with every
/// description array up to three entries over NULL, "x", another "x" and
/// "y", and every flag.
#[test]
fn unchanged_same_set_every_small_case_matches_c() {
    let strings = Strings::new();
    let pool = pool();
    // Three distinct combinations keep the case count down; the pool's
    // other entries are covered without a session and by the proptests.
    let combos = sequences(&pool[..3], 2);
    let picks = sequences(&[None, Some(1), Some(2), Some(3)], 3);
    let descs: Vec<Vec<*const c_char>> = picks.iter().map(|p| strings.ptrs(p)).collect();
    let declined = [pool[0]];
    for flags in [1u8, 3, 5, 7] {
        for current in &combos {
            for cd in &descs {
                for bound in &combos {
                    for bd in &descs {
                        check_unchanged(flags, current, cd, bound, bd, &declined, false);
                    }
                }
            }
        }
    }
}

/// Every pair of descriptions from the whole pool, on one and two
/// combinations, both orders.
#[test]
fn unchanged_description_bytes_match_c() {
    let strings = Strings::new();
    let pool = pool();
    let mut all: Vec<Option<usize>> = (0..strings.0.len()).map(Some).collect();
    all.push(None);
    for &a in &all {
        for &b in &all {
            for &c in &all {
                let one = [pool[0]];
                check_unchanged(
                    1,
                    &one,
                    &strings.ptrs(&[a]),
                    &one,
                    &strings.ptrs(&[b]),
                    &[],
                    false,
                );
                let two = [pool[0], pool[1]];
                let swapped = [pool[1], pool[0]];
                check_unchanged(
                    1,
                    &two,
                    &strings.ptrs(&[a, b]),
                    &swapped,
                    &strings.ptrs(&[c, a]),
                    &[],
                    false,
                );
            }
        }
    }
}

/// Without a session: every current and declined set up to three entries
/// over the whole pool, with every retry and partly-declined flag.
#[test]
fn unchanged_without_session_every_small_case_matches_c() {
    let strings = Strings::new();
    let pool = pool();
    let sets = sequences(&pool, 3);
    let cd = strings.ptrs(&[Some(1), None]);
    let bound = [pool[1]];
    for flags in [0u8, 2, 4, 6] {
        for current in &sets {
            for declined in &sets {
                check_unchanged(flags, current, &cd, &bound, &[], declined, false);
            }
        }
    }
    // NULL arrays for the empty views, as the portal client passes them.
    for flags in 0..8u8 {
        for current in sequences(&pool, 1) {
            check_unchanged(flags, &current, &[], &[], &[], &[], true);
        }
    }
}

#[test]
fn request_every_small_case_matches_c() {
    let pool = pool();
    let current_sets = sequences(&pool, 4);
    let declined_sets = sequences(&pool, 2);
    for current in &current_sets {
        for declined in &declined_sets {
            for partly in [false, true] {
                check_request(current, partly, declined, false);
            }
        }
    }
    for partly in [false, true] {
        for declined in &declined_sets {
            check_request(&[], partly, declined, true);
        }
    }
}

#[test]
fn partition_every_small_case_matches_c() {
    let pool = pool();
    let sets3 = sequences(&pool[..4], 3);
    let sets2 = sequences(&pool[..4], 2);
    for current in &sets3 {
        for approved in &sets2 {
            for request in &sets3 {
                check_partition(current, approved, request, false);
            }
        }
    }
    for approved in &sets2 {
        for request in &sets2 {
            check_partition(&[], approved, request, true);
        }
    }
}

/// The intentional differences, on the Rust side only: null views and
/// null outputs C would dereference.
#[test]
fn rust_reads_null_views_as_empty_and_skips_null_outputs() {
    let a = [kc(0, KEY_A)];
    let bogus = obs_nix_portal_combos {
        array: std::ptr::null(),
        num: 3,
    };
    let full = obs_nix_portal_combos {
        array: a.as_ptr(),
        num: 1,
    };
    let x = CString::new("x").unwrap();
    let xp = [x.as_ptr()];
    let full_desc = desc_rust(&xp, false);
    let bogus_desc = obs_nix_portal_descriptions {
        array: std::ptr::null(),
        num: 2,
    };
    // SAFETY: the shims accept null views and null outputs.
    unsafe {
        assert!(!obs_nix_portal_sets_has(std::ptr::null(), a[0]));
        assert!(!obs_nix_portal_sets_has(&raw const bogus, a[0]));
        // Two empty sets are the same, without a session nothing to bind.
        assert!(obs_nix_portal_sets_unchanged(
            true,
            false,
            false,
            std::ptr::null(),
            std::ptr::null(),
            &raw const bogus,
            &raw const bogus_desc,
            std::ptr::null(),
        ));
        assert!(obs_nix_portal_sets_unchanged(
            false,
            false,
            false,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
        ));
        // A missing description is never the same.
        assert!(!obs_nix_portal_sets_unchanged(
            true,
            false,
            false,
            &raw const full,
            &raw const bogus_desc,
            &raw const full,
            &raw const full_desc,
            std::ptr::null(),
        ));
        // Null outputs are skipped; the results stay.
        assert!(!obs_nix_portal_sets_request(
            &raw const full,
            false,
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ));
        assert!(obs_nix_portal_sets_request(
            &raw const full,
            true,
            &raw const full,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ));
        let two = [kc(0, KEY_A), kc(0, KEY_NONE)];
        let current = obs_nix_portal_combos {
            array: two.as_ptr(),
            num: 2,
        };
        assert!(obs_nix_portal_sets_partition(
            &raw const current,
            &raw const full,
            &raw const current,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ));
    }
}

fn arb_combo() -> impl Strategy<Value = obs_key_combination> {
    prop_oneof![
        4 => (0usize..6).prop_map(|i| pool()[i]),
        1 => (any::<u32>(), any::<i32>()).prop_map(|(m, k)| kc(m, k)),
        1 => (prop::sample::select(vec![0, CAPS, INTERACT_CONTROL_KEY, 1 << 9]), 0i32..4)
            .prop_map(|(m, k)| kc(m, k)),
    ]
}

fn arb_set() -> impl Strategy<Value = Vec<obs_key_combination>> {
    prop::collection::vec(arb_combo(), 0..9)
}

/// Description picks: an index into the pool, a NULL, or random bytes
/// (index past the pool: into `extra`).
fn arb_picks() -> impl Strategy<Value = Vec<Option<usize>>> {
    prop::collection::vec(prop::option::weighted(0.85, 0usize..14), 0..10)
}

fn arb_extra() -> impl Strategy<Value = Vec<Vec<u8>>> {
    prop::collection::vec(prop::collection::vec(1u8..=255, 0..4), 3)
}

fn strings_with(extra: &[Vec<u8>]) -> Strings {
    let mut s = Strings::new();
    s.0.extend(extra.iter().map(|b| CString::new(b.clone()).unwrap()));
    s
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(3000)))]

    #[test]
    fn has_matches_c(set in arb_set(), probe in arb_combo(), null: bool) {
        check_has(&set, probe, null);
    }

    #[test]
    fn unchanged_matches_c(
        flags in 0u8..8,
        current in arb_set(),
        bound in arb_set(),
        declined in arb_set(),
        cp in arb_picks(),
        bp in arb_picks(),
        extra in arb_extra(),
        reuse in prop::option::of(any::<prop::sample::Index>()),
        null: bool,
    ) {
        let strings = strings_with(&extra);
        // Sometimes bound is current reordered, so the descriptions decide.
        let (bound, bp) = match reuse {
            Some(ix) if !current.is_empty() => {
                let mut b = current.clone();
                b.rotate_left(ix.index(current.len()));
                let mut p = cp.clone();
                if !p.is_empty() {
                    let n = ix.index(current.len()) % p.len();
                    p.rotate_left(n);
                }
                (b, p)
            }
            _ => (bound, bp),
        };
        check_unchanged(
            flags,
            &current,
            &strings.ptrs(&cp),
            &bound,
            &strings.ptrs(&bp),
            &declined,
            null,
        );
    }

    #[test]
    fn request_matches_c(current in arb_set(), partly: bool, declined in arb_set(), null: bool) {
        check_request(&current, partly, &declined, null);
    }

    #[test]
    fn partition_matches_c(
        current in arb_set(),
        approved in arb_set(),
        request in arb_set(),
        null: bool,
    ) {
        check_partition(&current, &approved, &request, null);
    }
}
