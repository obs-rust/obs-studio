//! Tier 1: a 1:1 port of `test/cmocka/test_output_protocol_list_steps.c`
//! against the safe core [`obs_output::protocols`], plus edge cases.
//!
//! The C test calls `obs_output_protocol_list_count()` with C strings; the
//! core takes the bytes before their NULs, so `count(b"RTMP;RTMPS", b"RTMP")`
//! here is `count("RTMP;RTMPS", "RTMP")` there. The nine cases keep the C
//! names, their order and their assertions. The C's NULL list has no slice:
//! it is the shim's, which gives 0 as the C does
//! (`protocols_parity::null_list_counts_zero`); here the empty list stands
//! next to it, as in the C case. The "characterized, not endorsed" cases are
//! kept as such.

use obs_output::protocols::{SEPARATOR, count_matches as count};
use proptest::prelude::*;

#[test]
fn null_and_empty_list() {
    // count(NULL, "RTMP") and count(NULL, ""): the shim's, see the header.
    assert_eq!(count(b"", b"RTMP"), 0);
    assert_eq!(count(b"", b""), 0);
}

#[test]
fn whole_segments() {
    assert_eq!(count(b"RTMP", b"RTMP"), 1);
    assert_eq!(count(b"RTMP;RTMPS", b"RTMP"), 1);
    assert_eq!(count(b"RTMP;RTMPS", b"RTMPS"), 1);
    assert_eq!(count(b"RTMPS;RTMP", b"RTMP"), 1);
    assert_eq!(count(b"SRT;RIST", b"RIST"), 1);
    // prefixes either way do not match
    assert_eq!(count(b"RTMPX", b"RTMP"), 0);
    assert_eq!(count(b"RTMP", b"RTMPX"), 0);
    assert_eq!(count(b"RTMP", b"RTM"), 0);
    assert_eq!(count(b"XRTMP", b"RTMP"), 0);
    // the separator is not part of a segment
    assert_eq!(count(b"RTMP;RTMPS", b"RTMP;RTMPS"), 0);
    assert_eq!(count(b"RTMP;", b"RTMP;"), 0);
    assert_eq!(count(b"A;B", b"A;"), 0);
}

#[test]
fn case_and_spaces() {
    assert_eq!(count(b"rtmp", b"RTMP"), 0);
    assert_eq!(count(b"RTMP", b"rtmp"), 0);
    assert_eq!(count(b"Rtmp;rtmp;RTMP", b"rtmp"), 1);
    // nothing is trimmed
    assert_eq!(count(b" RTMP", b"RTMP"), 0);
    assert_eq!(count(b"RTMP ", b"RTMP"), 0);
    assert_eq!(count(b"WHIP; RIST", b" RIST"), 1);
    assert_eq!(count(b"WHIP; RIST", b"RIST"), 0);
    assert_eq!(count(b"\tRTMP", b"RTMP"), 0);
}

/// Characterized, not endorsed: a protocol listed twice counts twice, so
/// `enum_cb` gets the same output id twice.
#[test]
fn duplicates() {
    assert_eq!(count(b"RTMP;RTMP", b"RTMP"), 2);
    assert_eq!(count(b"RTMP;RTMPS;RTMP", b"RTMP"), 2);
    assert_eq!(count(b"RTMP;RTMP;RTMP", b"RTMP"), 3);
    assert_eq!(count(b"RTMP;rtmp;RTMP", b"RTMP"), 2);
}

#[test]
fn empty_and_trailing_segments() {
    assert_eq!(count(b";RTMP;;", b"RTMP"), 1);
    assert_eq!(count(b"HLS;", b"HLS"), 1);
    assert_eq!(count(b";HLS", b"HLS"), 1);
    assert_eq!(count(b";;;", b"RTMP"), 0);
    assert_eq!(count(b";", b"RTMP"), 0);
}

/// Characterized, not endorsed: the empty protocol matches each empty
/// segment that a `;` ends, but not a trailing one (the C stops at the
/// NUL). libobs never asks for it: `""` is never a registered protocol, and
/// the caller returns before the match.
#[test]
fn empty_protocol() {
    assert_eq!(count(b";", b""), 1);
    assert_eq!(count(b";;", b""), 2);
    assert_eq!(count(b";RTMP;;", b""), 2);
    assert_eq!(count(b";A", b""), 1);
    assert_eq!(count(b"A;", b""), 0);
    assert_eq!(count(b"A;;B", b""), 1);
    assert_eq!(count(b"A", b""), 0);
}

/// Bytes, not text: high bytes, bytes that are not UTF-8, and control
/// bytes compare as they are.
#[test]
fn bytes() {
    assert_eq!(count(b"\xff;\xfe", b"\xff"), 1);
    assert_eq!(count(b"\xff;\xfe", b"\xfe"), 1);
    assert_eq!(count(b"\xff\xfe", b"\xff"), 0);
    assert_eq!(count(b"\xc3\xa9;e\xcc\x81", b"\xc3\xa9"), 1);
    assert_eq!(count(b"\xc3\xa9;e\xcc\x81", b"e\xcc\x81"), 1);
    assert_eq!(count(b"\x01\x7f;\x80", b"\x80"), 1);
    assert_eq!(count(b"%s;%n", b"%n"), 1);
}

#[test]
fn long_lists() {
    // 100000 copies of "X"
    let copies = 100_000;
    let mut list = b"X;".repeat(copies);
    list.pop();
    assert_eq!(count(&list, b"X"), copies);
    assert_eq!(count(&list, b"XX"), 0);

    // one 1 MiB segment, and the same protocol one byte shorter
    let len = 1024 * 1024;
    let mut big = vec![b'P'; len];
    big.extend_from_slice(b";Q");
    let protocol = vec![b'P'; len];
    assert_eq!(count(&big, &protocol), 1);
    assert_eq!(count(&big, &protocol[..len - 1]), 0);
}

/// `model_count`: split at every `;` up to the end, without the segment
/// after a trailing `;`, and count the segments equal to `protocol`.
fn model_count(list: &[u8], protocol: &[u8]) -> usize {
    let n = list.len();
    let mut matches = 0;
    let mut start = 0;
    while start < n {
        let mut end = start;
        while end < n && list[end] != b';' {
            end += 1;
        }
        if &list[start..end] == protocol {
            matches += 1;
        }
        start = end + 1;
    }
    matches
}

/// Every string of `len` bytes over `alphabet`, in the C's order (the
/// first byte varies fastest).
fn strings(alphabet: &[u8], len: usize) -> Vec<Vec<u8>> {
    let base = alphabet.len();
    let combos = base.pow(u32::try_from(len).unwrap());
    (0..combos)
        .map(|mut v| {
            (0..len)
                .map(|_| {
                    let b = alphabet[v % base];
                    v /= base;
                    b
                })
                .collect()
        })
        .collect()
}

/// Every list of up to 7 bytes over {'A', 'B', ';'} against every protocol
/// of up to 3 bytes over {'A', 'B', ';'}, the empty one included.
#[test]
fn exhaustive_small() {
    let alphabet = *b"AB;";
    let protocols: Vec<Vec<u8>> = (0..=3).flat_map(|n| strings(&alphabet, n)).collect();
    let mut checked = 0usize;
    for llen in 0..=7 {
        for list in strings(&alphabet, llen) {
            for protocol in &protocols {
                assert_eq!(
                    count(&list, protocol),
                    model_count(&list, protocol),
                    "list {:?} protocol {:?}",
                    list.escape_ascii().to_string(),
                    protocol.escape_ascii().to_string()
                );
                checked += 1;
            }
        }
    }
    // (3^8 - 1) / 2 lists times (3^4 - 1) / 2 protocols
    assert_eq!(checked, 3280 * 40);
}

// Edge cases beyond the C test.

#[test]
fn separator_is_semicolon() {
    assert_eq!(SEPARATOR, b';');
    // Other separators a list might plausibly use are segment bytes.
    assert_eq!(count(b"RTMP,SRT", b"RTMP"), 0);
    assert_eq!(count(b"RTMP,SRT", b"RTMP,SRT"), 1);
    assert_eq!(count(b"RTMP SRT", b"SRT"), 0);
    assert_eq!(count(b"RTMP\nSRT", b"SRT"), 0);
}

#[test]
fn position_in_list() {
    for list in [
        &b"RTMP;A;B"[..],
        b"A;RTMP;B",
        b"A;B;RTMP",
        b";;RTMP",
        b"RTMP;;",
    ] {
        assert_eq!(count(list, b"RTMP"), 1, "{}", list.escape_ascii());
    }
}

#[test]
fn single_segment_list_is_whole_name() {
    assert_eq!(count(b"R", b"R"), 1);
    assert_eq!(count(b"R", b"RR"), 0);
    assert_eq!(count(b"RR", b"R"), 0);
    assert_eq!(count(b"R;", b"R"), 1);
    assert_eq!(count(b";R", b"R"), 1);
}

#[test]
fn protocol_with_separator_never_matches() {
    for protocol in [&b";"[..], b";;", b"A;", b";A", b"A;B", b"RTMP;RTMP"] {
        for list in [
            &b";"[..],
            b";;",
            b";;;",
            b"A;B",
            b"A;;B",
            b"RTMP;RTMP",
            protocol,
        ] {
            assert_eq!(
                count(list, protocol),
                0,
                "list {} protocol {}",
                list.escape_ascii(),
                protocol.escape_ascii()
            );
        }
    }
}

/// Every non-NUL byte value compares as itself, and only as itself.
#[test]
fn every_byte_value() {
    for b in 1u8..=255 {
        if b == SEPARATOR {
            continue;
        }
        let one = [b];
        let list = [b, SEPARATOR, b, SEPARATOR, b.wrapping_add(1)];
        assert_eq!(count(&list, &one), 2, "byte {b:#04x}");
        assert_eq!(count(&one, &one), 1, "byte {b:#04x}");
        let other = [b ^ 0x20];
        if other[0] != b && other[0] != 0 {
            assert_eq!(count(&one, &other), 0, "byte {b:#04x}");
        }
    }
}

/// Runs of separators: n empty segments are ended by n separators, and the
/// one after the last separator is not scanned.
#[test]
fn separator_runs() {
    for n in 0..64usize {
        let list = vec![SEPARATOR; n];
        assert_eq!(count(&list, b""), n, "{n} separators");
        assert_eq!(count(&list, b"A"), 0, "{n} separators");
        let mut tail = list.clone();
        tail.push(b'A');
        assert_eq!(count(&tail, b""), n, "{n} separators then A");
        assert_eq!(count(&tail, b"A"), 1, "{n} separators then A");
    }
}

/// The core compares every byte as it is, a NUL included, though the shim
/// never passes one (the C string ends there).
#[test]
fn nul_byte_in_slice_is_a_byte() {
    assert_eq!(count(b"A\0B;A", b"A"), 1);
    assert_eq!(count(b"A\0B;A", b"A\0B"), 1);
    assert_eq!(count(b"\0", b"\0"), 1);
}

#[test]
fn long_protocol_against_short_list() {
    let protocol = vec![b'P'; 4096];
    assert_eq!(count(b"P", &protocol), 0);
    assert_eq!(count(b"", &protocol), 0);
    assert_eq!(count(&protocol, &protocol), 1);
}

fn byte_strategy() -> impl Strategy<Value = u8> {
    prop_oneof![
        3 => Just(b';'),
        3 => prop::sample::select(vec![b'A', b'B', b'a']),
        2 => 0x80u8..=0xff,
        1 => any::<u8>(),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    /// The core equals the model over weighted bytes (any value, a NUL
    /// included, non-UTF-8 sequences, many separators).
    #[test]
    fn matches_model(
        list in prop::collection::vec(byte_strategy(), 0..64),
        protocol in prop::collection::vec(byte_strategy(), 0..6),
    ) {
        prop_assert_eq!(count(&list, &protocol), model_count(&list, &protocol));
    }

    /// Appending a segment adds 1 if it is the protocol and 0 otherwise
    /// (for a non-empty protocol, which a trailing `;` cannot change).
    #[test]
    fn appending_a_segment(
        list in prop::collection::vec(byte_strategy(), 0..32),
        segment in prop::collection::vec(byte_strategy().prop_filter("no ;", |b| *b != b';'), 1..5),
        protocol in prop::collection::vec(byte_strategy().prop_filter("no ;", |b| *b != b';'), 1..5),
    ) {
        let before = count(&list, &protocol);
        let mut longer = list.clone();
        longer.push(SEPARATOR);
        longer.extend_from_slice(&segment);
        let added = usize::from(segment == protocol);
        // A list that ended in ';' already ended its last segment; one that
        // did not now ends it with the new ';', which counts it the same.
        prop_assert_eq!(count(&longer, &protocol), before + added);
    }

    /// A protocol taken from the list matches at least once, and the count
    /// never exceeds the number of segments.
    #[test]
    fn segment_of_list_matches(
        list in prop::collection::vec(byte_strategy(), 1..64),
        pick in any::<prop::sample::Index>(),
    ) {
        let segments: Vec<&[u8]> = list.split(|b| *b == b';').collect();
        let protocol = segments[pick.index(segments.len())];
        let n = count(&list, protocol);
        prop_assert!(n <= segments.len());
        if !protocol.is_empty() {
            prop_assert!(n >= 1);
        }
    }

    /// Raw bytes of every value, long lists, no panic.
    #[test]
    fn raw_bytes_never_panic(
        list in prop::collection::vec(any::<u8>(), 0..4000),
        protocol in prop::collection::vec(any::<u8>(), 0..8),
    ) {
        let _ = count(&list, &protocol);
    }
}
