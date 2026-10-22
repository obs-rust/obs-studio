//! Tier 3: the Rust shim `obs_output_protocol_list_count` (and the safe
//! core under it) against the original C, `libobs/obs-output-protocols.c.h`,
//! compiled as `oracle_obs_output_protocol_list_count` (obs-c-oracle,
//! oracle/output_protocols.c). Both take the same NUL-terminated strings;
//! the counts must be equal.
//!
//! Inputs:
//! - every list of up to 7 bytes over {'A', 'B', 'a', ';'} against every
//!   protocol of up to 3 bytes over the same alphabet, the empty protocol
//!   included: 21845 x 85 = 1856825 pairs (the C characterization's
//!   exhaustive check has 131200), and the NULL list against each protocol;
//! - every pair of the C characterization `test_output_protocol_list_steps`,
//!   and its 100000-segment list and 1 MiB segment;
//! - proptests: lists of segments taken from the protocol, its prefixes and
//!   extensions, case variants, empty segments and arbitrary non-NUL bytes
//!   (high bytes and non-UTF-8 sequences included), with leading, doubled
//!   and trailing ';', against protocols taken from the list, near it or
//!   random; and raw non-NUL bytes of every value.
//!
//! Intentional difference, excluded from the comparison: a NULL protocol.
//! The C's `strlen()` crashes on it; the shim returns 0
//! (`null_protocol_counts_zero`, Rust only). libobs never passes one:
//! `obs_enum_output_types_with_protocol()` returns for a NULL protocol
//! before the match (#1828). Nothing else is excluded.

use core::ffi::c_char;
use std::ffi::CString;
use std::ptr;

use obs_c_oracle::output_protocols::oracle_obs_output_protocol_list_count;
use obs_output::ffi::output_protocols::obs_output_protocol_list_count;
use obs_output::protocols::count_matches;
use proptest::prelude::*;

/// `bytes` and a NUL, for the C. `bytes` holds no NUL.
fn c_string(bytes: &[u8]) -> Vec<u8> {
    assert!(!bytes.contains(&0), "a C string holds no NUL");
    let mut s = bytes.to_vec();
    s.push(0);
    s
}

fn as_ptr(s: Option<&[u8]>) -> *const c_char {
    s.map_or(ptr::null(), |s| s.as_ptr().cast())
}

/// The shim's count. `list` and `protocol` are NUL-terminated (or None).
fn rust(list: Option<&[u8]>, protocol: Option<&[u8]>) -> usize {
    // SAFETY: each pointer is NULL or a NUL-terminated buffer that lives
    // for the call.
    unsafe { obs_output_protocol_list_count(as_ptr(list), as_ptr(protocol)) }
}

/// The C's count. `list` is NUL-terminated or None; `protocol` is
/// NUL-terminated (the C crashes on NULL).
fn oracle(list: Option<&[u8]>, protocol: &[u8]) -> usize {
    // SAFETY: `list` is NULL or NUL-terminated, `protocol` NUL-terminated;
    // both live for the call.
    unsafe { oracle_obs_output_protocol_list_count(as_ptr(list), as_ptr(Some(protocol))) }
}

/// Checks one pair: the shim, the C and the safe core agree. `list` and
/// `protocol` are without their NULs.
fn check(list: &[u8], protocol: &[u8]) -> usize {
    let (l, p) = (c_string(list), c_string(protocol));
    let want = oracle(Some(&l), &p);
    let got = rust(Some(&l), Some(&p));
    assert_eq!(
        got,
        want,
        "shim vs C: list {} protocol {}",
        list.escape_ascii(),
        protocol.escape_ascii()
    );
    assert_eq!(
        count_matches(list, protocol),
        want,
        "core vs C: list {} protocol {}",
        list.escape_ascii(),
        protocol.escape_ascii()
    );
    want
}

/// Every string of `len` bytes over `alphabet`, NUL-terminated.
fn strings(alphabet: &[u8], len: usize) -> Vec<Vec<u8>> {
    let base = alphabet.len();
    let combos = base.pow(u32::try_from(len).unwrap());
    (0..combos)
        .map(|mut v| {
            let mut s: Vec<u8> = (0..len)
                .map(|_| {
                    let b = alphabet[v % base];
                    v /= base;
                    b
                })
                .collect();
            s.push(0);
            s
        })
        .collect()
}

#[test]
fn exhaustive_small_alphabet() {
    let alphabet = *b"ABa;";
    let protocols: Vec<Vec<u8>> = (0..=3).flat_map(|n| strings(&alphabet, n)).collect();
    assert_eq!(protocols.len(), 85);
    let mut pairs = 0usize;
    let mut matched = 0usize;
    for llen in 0..=7 {
        for list in strings(&alphabet, llen) {
            let bytes = &list[..list.len() - 1];
            for protocol in &protocols {
                let want = oracle(Some(&list), protocol);
                let got = rust(Some(&list), Some(protocol));
                let core = count_matches(bytes, &protocol[..protocol.len() - 1]);
                if got != want || core != want {
                    panic!(
                        "list {} protocol {}: shim {got}, core {core}, C {want}",
                        bytes.escape_ascii(),
                        protocol[..protocol.len() - 1].escape_ascii()
                    );
                }
                pairs += 1;
                matched += usize::from(want > 0);
            }
        }
    }
    // (4^8 - 1) / 3 lists times (4^4 - 1) / 3 protocols
    assert_eq!(pairs, 21845 * 85);
    // The space is not trivially all zeros.
    assert!(matched > pairs / 100, "{matched} of {pairs} pairs matched");
}

#[test]
fn null_list_counts_zero() {
    let alphabet = *b"ABa;";
    for protocol in (0..=3).flat_map(|n| strings(&alphabet, n)) {
        assert_eq!(oracle(None, &protocol), 0);
        assert_eq!(rust(None, Some(&protocol)), 0);
    }
    for protocol in [&b"RTMP"[..], b"", b"\xff"] {
        let p = c_string(protocol);
        assert_eq!(oracle(None, &p), 0);
        assert_eq!(rust(None, Some(&p)), 0);
    }
}

/// The intentional difference: the C crashes on a NULL protocol, the shim
/// counts 0, with or without a list.
#[test]
fn null_protocol_counts_zero() {
    assert_eq!(rust(None, None), 0);
    for list in [&b""[..], b"RTMP", b";", b";;", b"RTMP;RTMP"] {
        let l = c_string(list);
        assert_eq!(rust(Some(&l), None), 0, "{}", list.escape_ascii());
    }
}

/// Every pair `test_output_protocol_list_steps` asks for, with the count
/// it expects.
#[test]
fn characterization_pairs() {
    let pairs: &[(&[u8], &[u8], usize)] = &[
        (b"", b"RTMP", 0),
        (b"", b"", 0),
        (b"RTMP", b"RTMP", 1),
        (b"RTMP;RTMPS", b"RTMP", 1),
        (b"RTMP;RTMPS", b"RTMPS", 1),
        (b"RTMPS;RTMP", b"RTMP", 1),
        (b"SRT;RIST", b"RIST", 1),
        (b"RTMPX", b"RTMP", 0),
        (b"RTMP", b"RTMPX", 0),
        (b"RTMP", b"RTM", 0),
        (b"XRTMP", b"RTMP", 0),
        (b"RTMP;RTMPS", b"RTMP;RTMPS", 0),
        (b"RTMP;", b"RTMP;", 0),
        (b"A;B", b"A;", 0),
        (b"rtmp", b"RTMP", 0),
        (b"RTMP", b"rtmp", 0),
        (b"Rtmp;rtmp;RTMP", b"rtmp", 1),
        (b" RTMP", b"RTMP", 0),
        (b"RTMP ", b"RTMP", 0),
        (b"WHIP; RIST", b" RIST", 1),
        (b"WHIP; RIST", b"RIST", 0),
        (b"\tRTMP", b"RTMP", 0),
        (b"RTMP;RTMP", b"RTMP", 2),
        (b"RTMP;RTMPS;RTMP", b"RTMP", 2),
        (b"RTMP;RTMP;RTMP", b"RTMP", 3),
        (b"RTMP;rtmp;RTMP", b"RTMP", 2),
        (b";RTMP;;", b"RTMP", 1),
        (b"HLS;", b"HLS", 1),
        (b";HLS", b"HLS", 1),
        (b";;;", b"RTMP", 0),
        (b";", b"RTMP", 0),
        (b";", b"", 1),
        (b";;", b"", 2),
        (b";RTMP;;", b"", 2),
        (b";A", b"", 1),
        (b"A;", b"", 0),
        (b"A;;B", b"", 1),
        (b"A", b"", 0),
        (b"\xff;\xfe", b"\xff", 1),
        (b"\xff;\xfe", b"\xfe", 1),
        (b"\xff\xfe", b"\xff", 0),
        (b"\xc3\xa9;e\xcc\x81", b"\xc3\xa9", 1),
        (b"\xc3\xa9;e\xcc\x81", b"e\xcc\x81", 1),
        (b"\x01\x7f;\x80", b"\x80", 1),
        (b"%s;%n", b"%n", 1),
    ];
    for &(list, protocol, want) in pairs {
        assert_eq!(
            check(list, protocol),
            want,
            "list {} protocol {}",
            list.escape_ascii(),
            protocol.escape_ascii()
        );
    }
}

#[test]
fn long_lists() {
    let copies = 100_000;
    let mut list = b"X;".repeat(copies);
    list.pop();
    assert_eq!(check(&list, b"X"), copies);
    assert_eq!(check(&list, b"XX"), 0);
    assert_eq!(check(&list, b""), 0);
    list.push(b';');
    assert_eq!(check(&list, b"X"), copies);

    let len = 1024 * 1024;
    let mut big = vec![b'P'; len];
    big.extend_from_slice(b";Q");
    let protocol = vec![b'P'; len];
    assert_eq!(check(&big, &protocol), 1);
    assert_eq!(check(&big, &protocol[..len - 1]), 0);
    assert_eq!(check(&big, b"Q"), 1);
}

/// Every non-NUL byte value as a one-byte protocol against lists of it,
/// its neighbours and separators.
#[test]
fn every_byte_value() {
    for b in 1u8..=255 {
        let near = b.wrapping_add(1).max(1);
        for list in [
            vec![b],
            vec![b, b],
            vec![b, b';', b],
            vec![b';', b, b';'],
            vec![near, b';', b],
            vec![(b ^ 0x20) | 1, b';', b, b, b';'],
        ] {
            check(&list, &[b]);
            check(&list, &[b, b]);
            check(&list, b"");
        }
    }
}

fn non_nul() -> impl Strategy<Value = u8> {
    1u8..=255
}

/// A list built around `protocol`: segments that equal it, prefixes and
/// extensions of it, case variants, empty segments and random non-NUL
/// bytes, joined by ';', with an optional leading and trailing ';'.
fn list_around(protocol: Vec<u8>) -> impl Strategy<Value = (Vec<u8>, Vec<u8>)> {
    let p = protocol.clone();
    let segment = prop_oneof![
        3 => Just(p.clone()),
        1 => (0..=p.len()).prop_map({
            let p = p.clone();
            move |n| p[..n].to_vec()
        }),
        1 => non_nul().prop_map({
            let p = p.clone();
            move |b| {
                let mut s = p.clone();
                s.push(b);
                s
            }
        }),
        1 => non_nul().prop_map({
            let p = p.clone();
            move |b| {
                let mut s = vec![b];
                s.extend_from_slice(&p);
                s
            }
        }),
        1 => Just(p.to_ascii_lowercase()),
        1 => Just(p.to_ascii_uppercase()),
        1 => Just(Vec::new()),
        2 => prop::collection::vec(non_nul(), 0..6),
        1 => prop::collection::vec(0x80u8..=0xff, 1..4),
    ];
    (
        prop::collection::vec(segment, 0..12),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(move |(segments, lead, trail)| {
            let mut list = Vec::new();
            if lead {
                list.push(b';');
            }
            list.extend_from_slice(&segments.join(&b';'));
            if trail {
                list.push(b';');
            }
            (list, protocol.clone())
        })
}

/// Raw bytes of every value as a list: a NUL becomes ';', and so does the
/// byte at each of `cuts`, so segments stay short enough to repeat.
fn raw_list((bytes, cuts): (Vec<u8>, Vec<prop::sample::Index>)) -> Vec<u8> {
    let mut list: Vec<u8> = bytes
        .into_iter()
        .map(|b| if b == 0 { b';' } else { b })
        .collect();
    if !list.is_empty() {
        for cut in cuts {
            let at = cut.index(list.len());
            list[at] = b';';
        }
    }
    list
}

fn protocol_strategy() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        3 => prop::sample::select(vec![
            b"RTMP".to_vec(),
            b"RTMPS".to_vec(),
            b"SRT".to_vec(),
            b"RIST".to_vec(),
            b"WHIP".to_vec(),
            b"HLS".to_vec(),
            b" RIST".to_vec(),
            b"rtmp".to_vec(),
            b"\xff".to_vec(),
            b"\xc3\xa9".to_vec(),
            Vec::new(),
        ]),
        2 => prop::collection::vec(non_nul(), 0..8),
        1 => prop::collection::vec(0x80u8..=0xff, 1..5),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    /// Lists built around the protocol, against the protocol itself and
    /// against protocols near it.
    #[test]
    fn lists_around_a_protocol(
        (list, protocol) in protocol_strategy().prop_flat_map(list_around),
        extra in non_nul(),
    ) {
        check(&list, &protocol);
        let mut longer = protocol.clone();
        longer.push(extra);
        check(&list, &longer);
        if let Some((_, shorter)) = protocol.split_last() {
            check(&list, shorter);
        }
        check(&list, &protocol.to_ascii_lowercase());
        check(&list, b"");
    }

    /// Raw non-NUL bytes of every value, against a segment of the list, a
    /// slice of it that crosses a ';', or random bytes.
    #[test]
    fn raw_bytes(
        list in (
            prop::collection::vec(any::<u8>(), 0..2000),
            prop::collection::vec(any::<prop::sample::Index>(), 0..400),
        )
            .prop_map(raw_list),
        random in prop::collection::vec(non_nul(), 0..8),
        pick in any::<prop::sample::Index>(),
        start in any::<prop::sample::Index>(),
        len in 0usize..8,
    ) {
        check(&list, &random);
        let segments: Vec<&[u8]> = list.split(|b| *b == b';').collect();
        check(&list, segments[pick.index(segments.len())]);
        if !list.is_empty() {
            let s = start.index(list.len());
            let e = (s + len).min(list.len());
            check(&list, &list[s..e]);
        }
    }
}

/// `CString` round trip: the shim reads to the NUL, as the C does, so a
/// list in a longer buffer ends at its first NUL on both sides.
#[test]
fn reads_to_the_first_nul() {
    let buf = b"RTMP\0RTMP;RTMP\0";
    let p = CString::new("RTMP").unwrap();
    let p = p.as_bytes_with_nul();
    assert_eq!(oracle(Some(buf), p), 1);
    assert_eq!(rust(Some(buf), Some(p)), 1);
    let p = b"RTMP\0RTMPS\0";
    assert_eq!(oracle(Some(b"RTMP;RTMPS\0"), p), 1);
    assert_eq!(rust(Some(b"RTMP;RTMPS\0"), Some(p)), 1);
}
