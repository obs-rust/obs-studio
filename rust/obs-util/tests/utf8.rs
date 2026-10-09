//! Tier 1: safe-core tests. The `test_utf8_*` tests mirror
//! `test/cmocka/test_utf8.c` case for case. The core takes slices, so the C
//! cases that depend on `insize == 0` (NUL-terminated input) slice at the
//! first NUL here; the NULL-pointer cases are covered by the shim tests in
//! `utf8_parity.rs`.

use obs_c_oracle as _; // links the test allocator
use obs_util::utf8::{
    UTF8_IGNORE_ERROR, UTF8_SKIP_BOM, has_utf8_bom, utf8_to_wchar, wchar_to_utf8,
};

/// Wide string from ASCII.
fn w(s: &str) -> Vec<u32> {
    s.chars().map(u32::from).collect()
}

/// The part of `s` before the first NUL (what `insize == 0` converts).
fn nul_terminated(s: &[u8]) -> &[u8] {
    &s[..s.iter().position(|&b| b == 0).unwrap_or(s.len())]
}

#[test]
fn test_utf8_ascii_round_trip() {
    let input = b"hello";
    let mut out = [0u32; 8];

    assert_eq!(utf8_to_wchar(nul_terminated(input), None, 0), 5);
    assert_eq!(utf8_to_wchar(nul_terminated(input), Some(&mut out), 0), 5);
    assert_eq!(out[..5], w("hello")[..]);

    let mut back = [0u8; 8];
    let wide = &out[..out.iter().position(|&c| c == 0).unwrap()];
    assert_eq!(wchar_to_utf8(wide, None, 0), 5);
    assert_eq!(wchar_to_utf8(wide, Some(&mut back), 0), 5);
    assert_eq!(nul_terminated(&back), b"hello");
}

#[test]
fn test_utf8_to_wchar_multibyte() {
    let mut out = [0u32; 4];

    assert_eq!(utf8_to_wchar(b"\xc3\xa9", Some(&mut out), 0), 1);
    assert_eq!(out[0], 0xE9);

    assert_eq!(utf8_to_wchar(b"\xe2\x82\xac", Some(&mut out), 0), 1);
    assert_eq!(out[0], 0x20AC);

    assert_eq!(utf8_to_wchar(b"\xf0\x9f\x98\x80", Some(&mut out), 0), 1);
    assert_eq!(out[0], 0x1F600);
}

#[test]
fn test_utf8_wchar_to_utf8_multibyte() {
    let mut out = [0u8; 8];

    assert_eq!(wchar_to_utf8(&[0xE9], Some(&mut out), 0), 2);
    assert_eq!(&out[..2], b"\xc3\xa9");

    out = [0; 8];
    assert_eq!(wchar_to_utf8(&[0x20AC], Some(&mut out), 0), 3);
    assert_eq!(&out[..3], b"\xe2\x82\xac");

    out = [0; 8];
    assert_eq!(wchar_to_utf8(&[0x1F600], Some(&mut out), 0), 4);
    assert_eq!(&out[..4], b"\xf0\x9f\x98\x80");
}

#[test]
fn test_utf8_errors() {
    let mut out = [0u32; 8];
    let mut cout = [0u8; 8];

    // forbidden octets
    assert_eq!(utf8_to_wchar(b"\xc0\x80", Some(&mut out), 0), 0);
    assert_eq!(utf8_to_wchar(b"\xff", Some(&mut out), 0), 0);
    // truncated sequence
    assert_eq!(utf8_to_wchar(b"\xe2\x82", Some(&mut out), 0), 0);
    // bad continuation
    assert_eq!(utf8_to_wchar(b"\xe2\x41\x41", Some(&mut out), 0), 0);
    // surrogate
    assert_eq!(wchar_to_utf8(&[0xD800], Some(&mut cout), 0), 0);
}

#[test]
fn test_utf8_ignore_error() {
    let mut out = [0u32; 8];
    let mut cout = [0u8; 8];

    assert_eq!(
        utf8_to_wchar(b"a\xffb", Some(&mut out), UTF8_IGNORE_ERROR),
        2
    );
    assert_eq!(out[..2], w("ab")[..]);

    assert_eq!(
        wchar_to_utf8(&[0x61, 0xD800, 0x62], Some(&mut cout), UTF8_IGNORE_ERROR),
        2
    );
    assert_eq!(&cout[..2], b"ab");
}

#[test]
fn test_utf8_skip_bom() {
    let mut out = [0u32; 8];

    assert_eq!(
        utf8_to_wchar(b"\xef\xbb\xbfx", Some(&mut out), UTF8_SKIP_BOM),
        1
    );
    assert_eq!(out[0], u32::from(b'x'));

    out = [0; 8];
    assert_eq!(utf8_to_wchar(b"\xef\xbb\xbfx", Some(&mut out), 0), 2);
    assert_eq!(out[0], 0xFEFF);
    assert_eq!(out[1], u32::from(b'x'));
}

#[test]
fn test_utf8_invalid_arguments() {
    let mut out = [0u32; 8];
    let mut cout = [0u8; 8];

    // empty output buffer (the C `outsize == 0 && out != NULL` case)
    assert_eq!(utf8_to_wchar(b"abc", Some(&mut []), 0), 0);
    // output buffer too small
    assert_eq!(utf8_to_wchar(b"abc", Some(&mut out[..2]), 0), 0);

    assert_eq!(wchar_to_utf8(&w("abc"), Some(&mut []), 0), 0);
    assert_eq!(wchar_to_utf8(&w("abc"), Some(&mut cout[..2]), 0), 0);
}

#[test]
fn test_utf8_embedded_nul() {
    let mut out = [0u32; 8];

    // insize 0 stops at NUL
    assert_eq!(utf8_to_wchar(nul_terminated(b"a\0b"), None, 0), 1);
    // insize > 0 translates NUL as a regular symbol
    assert_eq!(utf8_to_wchar(b"a\0b", Some(&mut out), 0), 3);
    assert_eq!(out[0], u32::from(b'a'));
    assert_eq!(out[1], 0);
    assert_eq!(out[2], u32::from(b'b'));
}

// Edge cases beyond the C tests.

#[test]
fn utf8_empty_input() {
    let mut out = [0u32; 2];
    assert_eq!(utf8_to_wchar(b"", None, 0), 0);
    assert_eq!(utf8_to_wchar(b"", Some(&mut out), 0), 0);
    assert_eq!(wchar_to_utf8(&[], None, 0), 0);
}

#[test]
fn utf8_truncated_sequences() {
    for seq in [
        &b"\xc3"[..],
        b"\xe2\x82",
        b"\xf0\x9f\x98",
        b"a\xf0\x9f",
        b"\xf8\x88\x80\x80",
        b"\xfd\xbf\xbf\xbf\xbf",
    ] {
        assert_eq!(utf8_to_wchar(seq, None, 0), 0, "{seq:x?}");
        assert_eq!(utf8_to_wchar(seq, Some(&mut [0; 8]), 0), 0, "{seq:x?}");
    }
    // ignoring errors skips each stray byte
    let mut out = [0u32; 8];
    assert_eq!(
        utf8_to_wchar(b"a\xf0\x9f\x98", Some(&mut out), UTF8_IGNORE_ERROR),
        1
    );
    assert_eq!(out[0], u32::from(b'a'));
}

#[test]
fn utf8_bad_continuation_ignored() {
    let mut out = [0u32; 8];
    assert_eq!(
        utf8_to_wchar(b"\xe2\x41\x41", Some(&mut out), UTF8_IGNORE_ERROR),
        2
    );
    assert_eq!(out[..2], w("AA")[..]);
    // stray continuation byte
    assert_eq!(
        utf8_to_wchar(b"a\x80b", Some(&mut out), UTF8_IGNORE_ERROR),
        2
    );
    assert_eq!(out[..2], w("ab")[..]);
}

#[test]
fn utf8_forbidden_octets() {
    for octet in [0xc0u8, 0xc1, 0xf5, 0xff] {
        assert_eq!(utf8_to_wchar(&[octet], None, 0), 0, "{octet:#x}");
        assert_eq!(utf8_to_wchar(b"a", None, 0), 1);
    }
    // with IGNORE_ERROR the overlong lead 0xc0 0x80 is decoded like C does
    let mut out = [0xaau32; 4];
    assert_eq!(
        utf8_to_wchar(b"\xc0\x80", Some(&mut out), UTF8_IGNORE_ERROR),
        1
    );
    assert_eq!(out[0], 0);
}

#[test]
fn utf8_overlong_encodings_accepted_like_c() {
    // Only the octets 0xc0/0xc1/0xf5/0xff are rejected; other overlong
    // forms decode, matching the C implementation.
    let mut out = [0xaau32; 4];
    assert_eq!(utf8_to_wchar(b"\xe0\x80\x80", Some(&mut out), 0), 1);
    assert_eq!(out[0], 0);
    assert_eq!(utf8_to_wchar(b"\xe0\x81\xbf", Some(&mut out), 0), 1);
    assert_eq!(out[0], 0x7f);
    assert_eq!(utf8_to_wchar(b"\xf0\x80\x80\x80", Some(&mut out), 0), 1);
    assert_eq!(out[0], 0);
}

#[test]
fn utf8_five_and_six_byte_sequences() {
    let mut out = [0u32; 2];
    assert_eq!(utf8_to_wchar(b"\xf8\x88\x80\x80\x80", Some(&mut out), 0), 1);
    assert_eq!(out[0], 0x20_0000);
    assert_eq!(
        utf8_to_wchar(b"\xfd\xbf\xbf\xbf\xbf\xbf", Some(&mut out), 0),
        1
    );
    assert_eq!(out[0], 0x7fff_ffff);

    let mut bytes = [0u8; 8];
    assert_eq!(wchar_to_utf8(&[0x20_0000], Some(&mut bytes), 0), 5);
    assert_eq!(&bytes[..5], b"\xf8\x88\x80\x80\x80");
    assert_eq!(wchar_to_utf8(&[0x7fff_ffff], Some(&mut bytes), 0), 6);
    assert_eq!(&bytes[..6], b"\xfd\xbf\xbf\xbf\xbf\xbf");
}

#[test]
fn utf8_surrogates() {
    // encoded surrogate D800 in UTF-8
    let mut out = [0u32; 4];
    assert_eq!(utf8_to_wchar(b"\xed\xa0\x80", Some(&mut out), 0), 0);
    // size-only mode does not check surrogates
    assert_eq!(utf8_to_wchar(b"\xed\xa0\x80", None, 0), 1);
    assert_eq!(
        utf8_to_wchar(b"a\xed\xa0\x80b", Some(&mut out), UTF8_IGNORE_ERROR),
        2
    );
    assert_eq!(out[..2], w("ab")[..]);

    let mut cout = [0u8; 8];
    assert_eq!(wchar_to_utf8(&[0xDFFF], Some(&mut cout), 0), 0);
    assert_eq!(wchar_to_utf8(&[0xD7FF], Some(&mut cout), 0), 3);
    assert_eq!(wchar_to_utf8(&[0xE000], Some(&mut cout), 0), 3);
}

#[test]
fn utf8_negative_wide_chars() {
    let mut cout = [0u8; 8];
    assert_eq!(wchar_to_utf8(&[0x8000_0000], Some(&mut cout), 0), 0);
    assert_eq!(wchar_to_utf8(&[u32::MAX], None, 0), 0);
    assert_eq!(
        wchar_to_utf8(&[0x61, 0x8000_0000], Some(&mut cout), UTF8_IGNORE_ERROR),
        1
    );
    assert_eq!(cout[0], b'a');
}

#[test]
fn utf8_bom_flags() {
    let mut out = [0u32; 8];
    // BOM only
    assert_eq!(
        utf8_to_wchar(b"\xef\xbb\xbf", Some(&mut out), UTF8_SKIP_BOM),
        0
    );
    // BOM in the middle is skipped too
    assert_eq!(
        utf8_to_wchar(b"a\xef\xbb\xbfb", Some(&mut out), UTF8_SKIP_BOM),
        2
    );
    assert_eq!(out[..2], w("ab")[..]);
    // size-only mode ignores UTF8_SKIP_BOM
    assert_eq!(utf8_to_wchar(b"\xef\xbb\xbfx", None, UTF8_SKIP_BOM), 2);

    let mut cout = [0u8; 8];
    assert_eq!(wchar_to_utf8(&[0xFEFF, 0x78], Some(&mut cout), 0), 4);
    assert_eq!(&cout[..4], b"\xef\xbb\xbfx");
    cout = [0; 8];
    assert_eq!(
        wchar_to_utf8(&[0xFEFF, 0x78], Some(&mut cout), UTF8_SKIP_BOM),
        1
    );
    assert_eq!(cout[0], b'x');
    // size-only mode honours UTF8_SKIP_BOM for wchar_to_utf8
    assert_eq!(wchar_to_utf8(&[0xFEFF, 0x78], None, UTF8_SKIP_BOM), 1);
}

#[test]
fn utf8_has_bom() {
    assert!(has_utf8_bom(b"\xef\xbb\xbf"));
    assert!(has_utf8_bom(b"\xef\xbb\xbfabc"));
    assert!(!has_utf8_bom(b"\xef\xbb"));
    assert!(!has_utf8_bom(b"abc"));
    assert!(!has_utf8_bom(b""));
}

#[test]
fn utf8_output_buffer_too_small() {
    // exactly enough
    let mut out = [0u32; 3];
    assert_eq!(utf8_to_wchar(b"abc", Some(&mut out), 0), 3);
    // one short
    let mut out = [0u32; 2];
    assert_eq!(utf8_to_wchar(b"abc", Some(&mut out), 0), 0);
    // partial output is left behind, like C
    assert_eq!(out, [0x61, 0x62]);

    let mut cout = [0u8; 3];
    assert_eq!(wchar_to_utf8(&[0x20AC], Some(&mut cout), 0), 3);
    let mut cout = [0u8; 2];
    assert_eq!(wchar_to_utf8(&[0x20AC], Some(&mut cout), 0), 0);
    assert_eq!(cout, [0, 0]);
    // second character does not fit
    let mut cout = [0u8; 3];
    assert_eq!(wchar_to_utf8(&[0x61, 0x20AC], Some(&mut cout), 0), 0);
    assert_eq!(cout[0], b'a');
}

#[test]
fn utf8_round_trip_all_scalar_values() {
    for cp in (0..=0x10FFFFu32).filter(|c| !(0xD800..=0xDFFF).contains(c)) {
        let mut bytes = [0u8; 8];
        let n = wchar_to_utf8(&[cp], Some(&mut bytes), 0);
        assert_eq!(n, char::from_u32(cp).unwrap().len_utf8(), "{cp:#x}");
        let mut back = [0u32; 2];
        assert_eq!(utf8_to_wchar(&bytes[..n], Some(&mut back), 0), 1);
        assert_eq!(back[0], cp);
    }
}
