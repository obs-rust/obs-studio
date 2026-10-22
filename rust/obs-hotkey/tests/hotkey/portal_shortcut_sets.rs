//! Tier 1: the safe core of `libobs/obs-nix-portal-shortcut-sets.c.h`, the
//! combination set logic of the GlobalShortcuts portal client.
//!
//! The first twelve tests port `test/cmocka/test_portal_shortcut_sets.c`
//! one to one, under the same names, in the same order, with the same
//! inputs and expectations. A C description array is a slice of
//! `Option<&[u8]>` (`None` for a NULL pointer; a NULL array with no
//! entries is an empty slice). The C's position buffers have room for 8
//! and start as a sentinel, and the checks that nothing past current's
//! size is written are kept. The C test also checks that nothing is
//! allocated (`bnum_allocs()`): the core and the shims only borrow their
//! inputs and never call the allocator.
//!
//! The cases after those pin the edges of the safe API, and proptests the
//! invariants of each function.

use obs_hotkey::name_map::{KEY_NONE, OBS_KEY_MOUSE1, key_from_name};
use obs_hotkey::nix_hotkey_state::{INTERACT_CONTROL_KEY, INTERACT_SHIFT_KEY};
use obs_hotkey::portal_shortcut_sets::{
    KeyCombination, Partition, Request, has, partition, request, same_combo, unchanged,
};
use proptest::prelude::*;

/// `INTERACT_CAPS_KEY` (`obs-interaction.h`), a bit the portal does not know.
const INTERACT_CAPS_KEY: u32 = 1;

type Desc<'a> = Option<&'a [u8]>;

fn key(name: &str) -> i32 {
    let k = key_from_name(&std::ffi::CString::new(name).unwrap());
    assert_ne!(k, KEY_NONE, "{name} is a key");
    k
}

fn combo(modifiers: u32, key: i32) -> KeyCombination {
    KeyCombination { modifiers, key }
}

#[allow(non_snake_case)]
fn A() -> KeyCombination {
    combo(0, key("OBS_KEY_A"))
}
#[allow(non_snake_case)]
fn B() -> KeyCombination {
    combo(0, key("OBS_KEY_B"))
}
#[allow(non_snake_case)]
fn C() -> KeyCombination {
    combo(0, key("OBS_KEY_C"))
}
#[allow(non_snake_case)]
fn CTRL_A() -> KeyCombination {
    combo(INTERACT_CONTROL_KEY, key("OBS_KEY_A"))
}
#[allow(non_snake_case)]
fn SHIFT_A() -> KeyCombination {
    combo(INTERACT_SHIFT_KEY, key("OBS_KEY_A"))
}
#[allow(non_snake_case)]
fn CAPS_A() -> KeyCombination {
    combo(INTERACT_CAPS_KEY, key("OBS_KEY_A"))
}
#[allow(non_snake_case)]
fn NONE() -> KeyCombination {
    combo(0, KEY_NONE)
}
#[allow(non_snake_case)]
fn MOUSE1() -> KeyCombination {
    combo(0, OBS_KEY_MOUSE1)
}

fn d(s: &'static str) -> Desc<'static> {
    Some(s.as_bytes())
}

fn db(s: &'static [u8]) -> Desc<'static> {
    Some(s)
}

const NO_COMBOS: &[KeyCombination] = &[];
const NO_DESCRIPTIONS: &[Desc<'static>] = &[];

// ---------------------------------------------------------------------------
// obs_nix_portal_sets_has

#[test]
fn test_has() {
    assert!(!has(NO_COMBOS, A()));
    assert!(!has(NO_COMBOS, NONE()));

    let set = [B(), CTRL_A(), MOUSE1(), NONE()];
    assert!(has(&set, B()));
    assert!(has(&set, CTRL_A()));
    assert!(has(&set, MOUSE1()));
    assert!(has(&set, NONE()));

    // The same key with other modifiers is another combination.
    assert!(!has(&set, A()));
    assert!(!has(&set, SHIFT_A()));
    // Every modifier bit counts, not only the four the portal knows.
    assert!(!has(&set, CAPS_A()));
    let ctrl_caps_a = combo(INTERACT_CONTROL_KEY | INTERACT_CAPS_KEY, key("OBS_KEY_A"));
    assert!(!has(&set, ctrl_caps_a));

    // Only num entries are looked at.
    let first = &set[..1];
    assert!(has(first, B()));
    assert!(!has(first, CTRL_A()));
}

// ---------------------------------------------------------------------------
// obs_nix_portal_sets_unchanged with a session: the same set

/// unchanged(session, no retry) of current against bound.
fn same(a: &[KeyCombination], a_desc: &[Desc], b: &[KeyCombination], b_desc: &[Desc]) -> bool {
    // declined is not looked at with a session.
    let all = [A(), B(), C()];
    unchanged(true, false, false, a, a_desc, b, b_desc, &all)
}

#[test]
fn test_same_set_empty() {
    let one = [A()];
    let one_desc = [d("OBS: x")];

    assert!(same(NO_COMBOS, NO_DESCRIPTIONS, NO_COMBOS, NO_DESCRIPTIONS));
    assert!(!same(&one, &one_desc, NO_COMBOS, NO_DESCRIPTIONS));
    assert!(!same(NO_COMBOS, NO_DESCRIPTIONS, &one, &one_desc));
    // Descriptions without combinations change nothing.
    assert!(same(NO_COMBOS, &one_desc, NO_COMBOS, NO_DESCRIPTIONS));
}

#[test]
fn test_same_set_combinations() {
    let ab = [A(), B()];
    let ba = [B(), A()];
    let a_ctrl_a = [A(), CTRL_A()];
    let a_shift_a = [A(), SHIFT_A()];
    let abc = [A(), B(), C()];
    let xy = [d("x"), d("y")];
    let yx = [d("y"), d("x")];
    let xyz = [d("x"), d("y"), d("z")];

    assert!(same(&ab, &xy, &ab, &xy));
    // Order does not matter; each combination keeps its description.
    assert!(same(&ab, &xy, &ba, &yx));
    assert!(!same(&ab, &xy, &ba, &xy));
    // The same key with other modifiers.
    assert!(!same(&a_ctrl_a, &xy, &a_shift_a, &xy));
    // One more or one fewer.
    assert!(!same(&ab, &xy, &abc, &xyz));
    assert!(!same(&abc, &xyz, &ab, &xy));
}

/// Characterized, not endorsed: the combinations are compared as sets, so a
/// duplicate counts once; each copy is compared with the description of the
/// first copy on the other side.
#[test]
fn test_same_set_duplicates() {
    let a = [A()];
    let aa = [A(), A()];
    let aab = [A(), A(), B()];
    let ab = [A(), B()];
    let x = [d("x")];
    let xx = [d("x"), d("x")];
    let xy = [d("x"), d("y")];
    let yx = [d("y"), d("x")];
    let xxz = [d("x"), d("x"), d("z")];
    let xz = [d("x"), d("z")];

    assert!(same(&aa, &xx, &a, &x));
    assert!(same(&a, &x, &aa, &xx));
    assert!(same(&aab, &xxz, &ab, &xz));
    // The second copy has another description.
    assert!(!same(&aa, &xy, &a, &x));
    // Only the first copy on the other side counts.
    assert!(!same(&a, &x, &aa, &yx));
    assert!(same(&a, &x, &aa, &xy));
}

/// Fewer descriptions than combinations (the i < num guards): a combination
/// without one is never the same.
#[test]
fn test_same_set_short_descriptions() {
    let ab = [A(), B()];
    let ba = [B(), A()];
    let x = [d("x")];
    let xy = [d("x"), d("y")];
    let yx = [d("y"), d("x")];

    // Current has no description for B.
    assert!(!same(&ab, &x, &ab, &xy));
    // Bound has no description for B.
    assert!(!same(&ab, &xy, &ab, &x));
    // Bound's one description is for its first combination, B: A has none.
    assert!(!same(&ab, &xy, &ba, &x));
    assert!(same(&ab, &xy, &ba, &yx));
    // No descriptions at all.
    assert!(!same(&ab, NO_DESCRIPTIONS, &ab, NO_DESCRIPTIONS));
    assert!(!same(&ab, NO_DESCRIPTIONS, &ab, &xy));
}

/// A NULL description is never the same, not even as another NULL.
#[test]
fn test_same_set_null_description() {
    let a = [A()];
    let null: [Desc; 1] = [None];
    let x = [d("x")];
    let empty = [d("")];

    assert!(!same(&a, &null, &a, &x));
    assert!(!same(&a, &x, &a, &null));
    // Characterized, not endorsed.
    assert!(!same(&a, &null, &a, &null));
    // An empty description is a description.
    assert!(same(&a, &empty, &a, &empty));
    assert!(!same(&a, &empty, &a, &null));
    assert!(!same(&a, &empty, &a, &x));
}

/// Descriptions are compared byte by byte (strcmp): no Unicode
/// normalization, no case folding, and any bytes, valid UTF-8 or not.
#[test]
fn test_same_set_description_bytes() {
    let a = [A()];
    let nfc = [db(b"OBS: Mic \xc3\xa9")];
    let nfd = [db(b"OBS: Mic e\xcc\x81")];
    let nfc_copy = [db(b"OBS: Mic \xc3\xa9")];
    let upper = [d("OBS: MIC")];
    let lower = [d("OBS: mic")];
    let prefix = [d("OBS: Mic")];
    let longer = [d("OBS: Mic ")];
    let raw = [db(b"\xff\xfe\x80 not UTF-8")];
    let raw_copy = [db(b"\xff\xfe\x80 not UTF-8")];
    let raw_other = [db(b"\xff\xfe\x81 not UTF-8")];

    assert!(same(&a, &nfc, &a, &nfc_copy));
    assert!(!same(&a, &nfc, &a, &nfd));
    assert!(!same(&a, &upper, &a, &lower));
    assert!(!same(&a, &prefix, &a, &longer));
    assert!(!same(&a, &longer, &a, &prefix));
    assert!(same(&a, &raw, &a, &raw_copy));
    assert!(!same(&a, &raw, &a, &raw_other));
}

// ---------------------------------------------------------------------------
// obs_nix_portal_sets_unchanged without a session

fn unchanged_without_session(current: &[KeyCombination], declined: &[KeyCombination]) -> bool {
    // bound and the descriptions are not looked at without a session.
    let xyz = [d("x"), d("y"), d("z")];
    let c = [C()];
    unchanged(
        false,
        false,
        false,
        current,
        &xyz,
        &c,
        NO_DESCRIPTIONS,
        declined,
    )
}

#[test]
fn test_unchanged_without_session() {
    let a = [A()];
    let ab = [A(), B()];
    let ba = [B(), A()];
    let aa = [A(), A()];
    let abc = [A(), B(), C()];
    let ctrl_a = [CTRL_A()];

    // No bindings: nothing to bind.
    assert!(unchanged_without_session(NO_COMBOS, NO_COMBOS));
    assert!(unchanged_without_session(NO_COMBOS, &ab));
    // Nothing declined: bind.
    assert!(!unchanged_without_session(&a, NO_COMBOS));
    // Every combination declined before: not asked for again.
    assert!(unchanged_without_session(&a, &a));
    assert!(unchanged_without_session(&ab, &ba));
    assert!(unchanged_without_session(&a, &abc));
    assert!(unchanged_without_session(&aa, &a));
    // A new combination: asked for.
    assert!(!unchanged_without_session(&ab, &a));
    assert!(!unchanged_without_session(&a, &ctrl_a));
}

// ---------------------------------------------------------------------------
// obs_nix_portal_sets_unchanged: the flags

/// Every combination of session, retry and partly_declined, for a current
/// set that is the bound set and also all declined (so each branch alone
/// says unchanged), and for one that is neither.
#[test]
fn test_unchanged_flags() {
    let current = [A(), B()];
    let current_desc = [d("x"), d("y")];
    let other = [C()];
    let other_desc = [d("z")];

    for flags in 0..8 {
        let (session, retry, partly) = (flags & 1 != 0, flags & 2 != 0, flags & 4 != 0);
        // Only a retry of a partly declined session binds anew.
        let want = !(session && retry && partly);
        let got = unchanged(
            session,
            retry,
            partly,
            &current,
            &current_desc,
            &current,
            &current_desc,
            &current,
        );
        assert_eq!(
            got, want,
            "same set, session {session} retry {retry} partly {partly}"
        );
        let got = unchanged(
            session,
            retry,
            partly,
            &current,
            &current_desc,
            &other,
            &other_desc,
            &other,
        );
        assert!(
            !got,
            "other set, session {session} retry {retry} partly {partly}: unchanged"
        );
    }

    // No bindings and no session: unchanged, whatever the flags.
    for flags in 0..4 {
        let (retry, partly) = (flags & 1 != 0, flags & 2 != 0);
        assert!(unchanged(
            false,
            retry,
            partly,
            NO_COMBOS,
            NO_DESCRIPTIONS,
            &other,
            &other_desc,
            NO_COMBOS
        ));
    }
}

// ---------------------------------------------------------------------------
// obs_nix_portal_sets_request

const SENTINEL: usize = 0xdead_beef;

/// Asks for current with partly_declined and declined; checks the positions
/// and the return value, and that nothing past current's size was written.
#[track_caller]
fn expect_request(
    current: &[KeyCombination],
    partly_declined: bool,
    declined: &[KeyCombination],
    want: &[usize],
    want_partly: bool,
    what: &str,
) {
    let mut out = [SENTINEL; 8];
    let got = request(current, partly_declined, declined, &mut out);
    assert_eq!(&out[..got.num], want, "{what}");
    assert_eq!(got.partly, want_partly, "{what}: partly");
    assert!(
        out[current.len()..].iter().all(|&p| p == SENTINEL),
        "{what}: written past current"
    );
}

#[test]
fn test_request() {
    let abc = [A(), B(), C()];
    let b = [B()];
    let ca = [C(), A()];
    let cab = [C(), A(), B()];
    let aab = [A(), A(), B()];
    let a = [A()];
    let ctrl_a_a = [CTRL_A(), A()];
    let ctrl_a = [CTRL_A()];

    // Not partly declined: all of current, declined or not.
    expect_request(
        &abc,
        false,
        NO_COMBOS,
        &[0, 1, 2],
        false,
        "all, nothing declined",
    );
    expect_request(
        &abc,
        false,
        &abc,
        &[0, 1, 2],
        false,
        "all, declined ignored",
    );
    expect_request(NO_COMBOS, false, NO_COMBOS, &[], false, "empty");

    // Partly declined: the declined ones are left out, in current's order.
    expect_request(&abc, true, &b, &[0, 2], true, "B declined");
    expect_request(&abc, true, &ca, &[1], true, "C and A declined");
    expect_request(&abc, true, NO_COMBOS, &[0, 1, 2], false, "nothing declined");
    expect_request(&abc, true, &cab, &[], true, "everything declined");
    // Each copy of a declined duplicate.
    expect_request(&aab, true, &a, &[2], true, "duplicate declined");
    // The same key with other modifiers is not declined.
    expect_request(&ctrl_a_a, true, &ctrl_a, &[1], true, "Ctrl+A declined");
    expect_request(&ctrl_a_a, true, &a, &[0], true, "A declined");
    expect_request(NO_COMBOS, true, &a, &[], false, "empty, partly declined");
}

// ---------------------------------------------------------------------------
// obs_nix_portal_sets_partition

#[track_caller]
fn expect_partition(
    current: &[KeyCombination],
    approved: &[KeyCombination],
    request: &[KeyCombination],
    want_part: &[usize],
    want_declined: &[usize],
    want_bind: bool,
    what: &str,
) {
    let mut part = [SENTINEL; 8];
    let mut declined = [SENTINEL; 8];
    let got = partition(current, approved, request, &mut part, &mut declined);
    assert_eq!(&part[..got.num_part], want_part, "{what}: part");
    assert_eq!(
        &declined[..got.num_declined],
        want_declined,
        "{what}: declined"
    );
    assert_eq!(got.bind, want_bind, "{what}: bind");
    // Each has room for current's size, and nothing past it is written.
    assert!(
        part[current.len()..].iter().all(|&p| p == SENTINEL),
        "{what}: part written past current"
    );
    assert!(
        declined[current.len()..].iter().all(|&p| p == SENTINEL),
        "{what}: declined written past current"
    );
}

#[test]
fn test_partition() {
    let a = [A()];
    let b = [B()];
    let ab = [A(), B()];
    let ac = [A(), C()];
    let abc = [A(), B(), C()];
    let ca = [C(), A()];
    let ctrl_a = [CTRL_A()];
    let ctrl_a_b = [CTRL_A(), B()];

    // A approved before, B new and declined: A is bound again.
    expect_partition(&ab, &a, &ab, &[0], &[1], true, "new key declined");
    // Part keeps current's order, not approved's.
    expect_partition(&abc, &ca, &abc, &[0, 2], &[1], true, "order");
    // Nothing approved before: nothing to bind, all declined.
    expect_partition(&ab, NO_COMBOS, &ab, &[], &[0, 1], false, "nothing approved");
    expect_partition(
        &ab,
        &ctrl_a,
        &ab,
        &[],
        &[0, 1],
        false,
        "other modifiers approved",
    );
    // Every requested key approved before (only a description changed):
    // part is the whole request, so nothing is bound again.
    expect_partition(&ab, &ab, &ab, &[0, 1], &[], false, "whole request approved");
    // An approved key left out of the request (declined while others
    // stayed bound) is not in part.
    expect_partition(
        &abc,
        &ab,
        &ac,
        &[0],
        &[1, 2],
        true,
        "approved, not requested",
    );
    expect_partition(
        &ab,
        &ab,
        &b,
        &[1],
        &[0],
        false,
        "approved, not requested, rest of request",
    );
    // Nothing current.
    expect_partition(NO_COMBOS, &ab, &ab, &[], &[], false, "nothing current");
    // The same key with other modifiers.
    expect_partition(
        &ctrl_a_b,
        &ab,
        &ctrl_a_b,
        &[1],
        &[0],
        true,
        "Ctrl+A is not A",
    );
}

/// Characterized, not endorsed: |part| counts duplicates in current and
/// |request| is taken as given, also with combinations that are not in
/// current. The portal client never passes either: its bindings hold each
/// combination once, and its request is a subset of them.
#[test]
fn test_partition_counts() {
    let a = [A()];
    let ab = [A(), B()];
    let aab = [A(), A(), B()];
    let abc = [A(), B(), C()];

    // Two copies of approved A make part as large as the request.
    expect_partition(&aab, &a, &ab, &[0, 1], &[2], false, "duplicate approved");
    // A request with C, which is not current.
    expect_partition(&a, &a, &abc, &[0], &[], true, "request beyond current");
}

// ---------------------------------------------------------------------------
// Edges of the safe API

#[test]
fn same_combo_compares_every_bit() {
    for bit in 0..32 {
        let m = 1u32 << bit;
        assert!(same_combo(combo(m, 5), combo(m, 5)));
        assert!(!same_combo(combo(m, 5), combo(0, 5)), "bit {bit}");
        assert!(!same_combo(combo(m, 5), combo(m, 6)));
    }
    // Any key code, in range or not.
    for k in [i32::MIN, -1, 0, OBS_KEY_MOUSE1, i32::MAX] {
        assert!(same_combo(combo(u32::MAX, k), combo(u32::MAX, k)));
        assert!(!same_combo(combo(0, k), combo(0, k.wrapping_add(1))));
    }
}

/// The partition fills part with every approved position first and then
/// filters it in place, as the C does: the entries between num_part and the
/// number of approved positions keep approved positions that were filtered
/// out.
#[test]
fn partition_leaves_filtered_positions_after_part() {
    let abc = [A(), B(), C()];
    let mut part = [SENTINEL; 4];
    let mut declined = [SENTINEL; 4];
    // All three approved; only C requested.
    let got = partition(&abc, &abc, &[C()], &mut part, &mut declined);
    assert_eq!(
        got,
        Partition {
            num_part: 1,
            num_declined: 2,
            bind: false
        }
    );
    // Part is {2}, then the approved positions 1 and 2 from the first pass.
    assert_eq!(part, [2, 1, 2, SENTINEL]);
    assert_eq!(declined, [0, 1, SENTINEL, SENTINEL]);
}

/// A buffer shorter than current loses the positions past its end; the
/// counts and the results do not change.
#[test]
fn short_buffers_keep_the_counts() {
    let abc = [A(), B(), C()];
    let mut short = [SENTINEL; 1];
    assert_eq!(
        request(&abc, false, NO_COMBOS, &mut short),
        Request {
            num: 3,
            partly: false
        }
    );
    assert_eq!(short, [0]);
    assert_eq!(
        request(&abc, true, &[A()], &mut []),
        Request {
            num: 2,
            partly: true
        }
    );

    let mut part = [SENTINEL; 1];
    let got = partition(&abc, &[A(), B()], &abc, &mut part, &mut []);
    assert_eq!(
        got,
        Partition {
            num_part: 2,
            num_declined: 1,
            bind: true
        }
    );
    assert_eq!(part, [0]);
}

/// The descriptions of a set may be of any comparable type; NULL is `None`.
#[test]
fn descriptions_of_any_type() {
    let ab = [A(), B()];
    let ba = [B(), A()];
    let n: [Option<u32>; 2] = [Some(1), Some(2)];
    let m: [Option<u32>; 2] = [Some(2), Some(1)];
    assert!(unchanged(true, false, false, &ab, &n, &ba, &m, NO_COMBOS));
    assert!(!unchanged(true, false, false, &ab, &n, &ab, &m, NO_COMBOS));
    let none: [Option<u32>; 2] = [None, Some(2)];
    assert!(!unchanged(
        true, false, false, &ab, &none, &ab, &none, NO_COMBOS
    ));
}

// ---------------------------------------------------------------------------
// Properties

fn arb_combo() -> impl Strategy<Value = KeyCombination> {
    // A few keys (none included) and modifiers (stray bits included), so
    // sets overlap and repeat.
    (
        prop::sample::select(vec![0u32, INTERACT_CONTROL_KEY, INTERACT_CAPS_KEY, 1 << 31]),
        prop::sample::select(vec![KEY_NONE, 1, 2, 3, OBS_KEY_MOUSE1]),
    )
        .prop_map(|(modifiers, key)| combo(modifiers, key))
}

fn arb_set() -> impl Strategy<Value = Vec<KeyCombination>> {
    prop::collection::vec(arb_combo(), 0..6)
}

fn arb_descriptions() -> impl Strategy<Value = Vec<Option<Vec<u8>>>> {
    prop::collection::vec(
        prop::option::of(prop::sample::select(vec![
            b"".to_vec(),
            b"x".to_vec(),
            b"y".to_vec(),
            b"\xff".to_vec(),
        ])),
        0..7,
    )
}

fn as_desc(v: &[Option<Vec<u8>>]) -> Vec<Option<&[u8]>> {
    v.iter().map(|d| d.as_deref()).collect()
}

/// A reference model of the same set, written from the C's comments.
fn model_same(
    a: &[KeyCombination],
    ad: &[Option<&[u8]>],
    b: &[KeyCombination],
    bd: &[Option<&[u8]>],
) -> bool {
    fn first<'a>(
        set: &[KeyCombination],
        descs: &[Option<&'a [u8]>],
        c: KeyCombination,
    ) -> Option<&'a [u8]> {
        let i = set.iter().position(|x| *x == c)?;
        descs.get(i).copied().flatten()
    }
    a.iter().all(|c| b.contains(c))
        && b.iter().all(|c| a.contains(c))
        && a.iter().enumerate().all(|(i, c)| {
            let mine = ad.get(i).copied().flatten();
            matches!((mine, first(b, bd, *c)), (Some(x), Some(y)) if x == y)
        })
}

proptest! {
    #[test]
    fn has_is_contains(set in arb_set(), c in arb_combo()) {
        prop_assert_eq!(has(&set, c), set.contains(&c));
    }

    #[test]
    fn unchanged_matches_the_model(
        session: bool,
        retry: bool,
        partly: bool,
        current in arb_set(),
        cd in arb_descriptions(),
        bound in arb_set(),
        bd in arb_descriptions(),
        declined in arb_set(),
    ) {
        let (cd, bd) = (as_desc(&cd), as_desc(&bd));
        let want = if session && retry && partly {
            false
        } else if session {
            model_same(&current, &cd, &bound, &bd)
        } else {
            current.iter().all(|c| declined.contains(c))
        };
        prop_assert_eq!(
            unchanged(session, retry, partly, &current, &cd, &bound, &bd, &declined),
            want
        );
    }

    #[test]
    fn request_is_a_filter(current in arb_set(), partly: bool, declined in arb_set()) {
        let mut out = vec![SENTINEL; current.len() + 2];
        let got = request(&current, partly, &declined, &mut out);
        let want: Vec<usize> = (0..current.len())
            .filter(|&i| !partly || !declined.contains(&current[i]))
            .collect();
        prop_assert_eq!(&out[..got.num], &want[..]);
        prop_assert_eq!(got.partly, want.len() < current.len());
        prop_assert!(out[current.len()..].iter().all(|&p| p == SENTINEL));
    }

    #[test]
    fn partition_splits_current(
        current in arb_set(),
        approved in arb_set(),
        req in arb_set(),
    ) {
        let mut part = vec![SENTINEL; current.len() + 2];
        let mut declined = vec![SENTINEL; current.len() + 2];
        let got = partition(&current, &approved, &req, &mut part, &mut declined);
        let inside = |i: &usize| approved.contains(&current[*i]) && req.contains(&current[*i]);
        let want_part: Vec<usize> = (0..current.len()).filter(inside).collect();
        let want_declined: Vec<usize> = (0..current.len()).filter(|i| !inside(i)).collect();
        prop_assert_eq!(&part[..got.num_part], &want_part[..]);
        prop_assert_eq!(&declined[..got.num_declined], &want_declined[..]);
        prop_assert_eq!(got.num_part + got.num_declined, current.len());
        prop_assert_eq!(got.bind, got.num_part > 0 && got.num_part < req.len());
        // Then the approved positions the filter dropped, from the first pass.
        let approved_positions: Vec<usize> =
            (0..current.len()).filter(|&i| approved.contains(&current[i])).collect();
        prop_assert_eq!(
            &part[got.num_part..approved_positions.len()],
            &approved_positions[got.num_part..]
        );
        prop_assert!(part[approved_positions.len()..].iter().all(|&p| p == SENTINEL));
        prop_assert!(declined[current.len()..].iter().all(|&p| p == SENTINEL));
    }
}
