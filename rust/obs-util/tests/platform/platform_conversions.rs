//! Tier 1: 1:1 port of `test/cmocka/test_platform_conversions.c` (the string
//! conversions of `libobs/util/platform-conversions.c`, #262 slice 1). Each
//! test names the cmocka case it mirrors. The conversions are checked on the
//! safe core (`obs_util::platform_conversions`); a NULL `str` and the `_ptr`
//! forms' `bmalloc` and `*pstr` handling live in the C ABI shim, so those
//! asserts go through it.
//!
//! As in the cmocka test, `wchar_t` is 32 bits off Windows and 16 bits
//! (UTF-16) on Windows, and the cases where the Windows API and libobs's own
//! decoder differ are pinned per side. The process stays in the C locale, as
//! a C program does before `setlocale`; the UTF-8 locale cases run in a child
//! of the test binary that switches `LC_CTYPE`, and are skipped where no
//! UTF-8 locale is installed.
//!
//! New here: the exact-size `os_utf8_to_wcs` case, the intentional
//! difference from C. Text exactly `dst_size` wide characters long gives 0
//! and an empty string, as `os_wcs_to_utf8` does for text exactly `dst_size`
//! bytes long; C converts it in full and writes the NUL one element past the
//! buffer. Nothing here writes past `dst_size`.

use core::ffi::{CStr, c_char};
use core::ptr;

use obs_c_oracle as _; // links the test allocator
use obs_util::ffi::bmem::{bfree, bnum_allocs};
use obs_util::ffi::platform_conversions as shim;
use obs_util::platform_conversions::{
    WChar, mbs_to_utf8, mbs_to_wcs, mbs_to_wcs_ptr, utf8_to_mbs, utf8_to_wcs, utf8_to_wcs_ptr,
    wcs_to_mbs, wcs_to_mbs_ptr, wcs_to_utf8, wcs_to_utf8_ptr,
};

use crate::common;

/// `WIDE_UTF32`: one unit per code point.
const WIDE_UTF32: bool = size_of::<WChar>() == 4;
const XW: WChar = b'x' as WChar;
const GW: WChar = b'G' as WChar;

/// Wide units from code units.
fn w(units: &[u32]) -> Vec<WChar> {
    units.iter().map(|&u| u as WChar).collect()
}

/// Wide units from code units, with a NUL.
fn wz(units: &[u32]) -> Vec<WChar> {
    let mut v = w(units);
    v.push(0);
    v
}

/// `s` as `wchar_t` units (`L"..."`), without the NUL.
fn ws(s: &str) -> Vec<WChar> {
    if WIDE_UTF32 {
        s.chars().map(|c| u32::from(c) as WChar).collect()
    } else {
        s.encode_utf16().map(|u| u as WChar).collect()
    }
}

/// `s` as `wchar_t` units, with the NUL.
fn wsz(s: &str) -> Vec<WChar> {
    let mut v = ws(s);
    v.push(0);
    v
}

/// `w` holds exactly the units of `want`, then a NUL.
fn check_wide(buf: &[WChar], want: &[u32]) {
    assert_eq!(&buf[..want.len()], &w(want)[..]);
    assert_eq!(buf[want.len()], 0);
}

/// `s` holds exactly the bytes of `want`, then a NUL.
fn check_bytes(buf: &[u8], want: &[u8]) {
    assert_eq!(&buf[..want.len()], want);
    assert_eq!(buf[want.len()], 0);
}

/// `LC_CTYPE` to a UTF-8 locale; false where none is installed.
fn set_utf8_ctype() -> bool {
    [c"C.UTF-8", c"en_US.UTF-8", c".UTF8", c"UTF-8"]
        .iter()
        // SAFETY: a NUL-terminated name; the isolated child runs one test.
        .any(|name| !unsafe { libc::setlocale(libc::LC_CTYPE, name.as_ptr()) }.is_null())
}

/// A `_ptr` result: the `bmalloc`ed buffer up to its NUL, freed.
fn take_wide(p: *mut WChar) -> Option<Vec<WChar>> {
    if p.is_null() {
        return None;
    }
    let mut out = Vec::new();
    // SAFETY: the shims return NUL-terminated `bmalloc`ed strings.
    unsafe {
        while *p.add(out.len()) != 0 {
            out.push(*p.add(out.len()));
        }
        bfree(p.cast());
    }
    Some(out)
}

fn take_bytes(p: *mut c_char) -> Option<Vec<u8>> {
    if p.is_null() {
        return None;
    }
    // SAFETY: the shims return NUL-terminated `bmalloc`ed strings.
    let bytes = unsafe { CStr::from_ptr(p) }.to_bytes().to_vec();
    // SAFETY: `p` came from `bmalloc`.
    unsafe { bfree(p.cast()) };
    Some(bytes)
}

/// A `_ptr` allocator for the safe core: a `Vec` of `n` elements of `x`.
fn vec_alloc<'a, T: Copy>(buf: &'a mut Vec<T>, fill: T) -> impl FnOnce(usize) -> &'a mut [T] {
    move |n| {
        *buf = vec![fill; n];
        buf
    }
}

// --------------------------------------------------------------------------
// os_utf8_to_wcs

#[test]
fn test_utf8_to_wcs_ascii() {
    let hello = (*b"hello").map(u32::from);
    let mut buf = [XW; 16];

    // NULL dst: the number of units, without the NUL (dst_size is ignored)
    assert_eq!(utf8_to_wcs(b"hello", None), 5);

    assert_eq!(utf8_to_wcs(b"hello", Some(&mut buf)), 5);
    check_wide(&buf, &hello);
    assert_eq!(buf[6], XW);

    // an explicit len converts that many bytes
    buf.fill(XW);
    assert_eq!(utf8_to_wcs(&b"hello"[..3], None), 3);
    assert_eq!(utf8_to_wcs(&b"hello"[..3], Some(&mut buf)), 3);
    check_wide(&buf, &hello[..3]);

    // len 0 means strlen (the shim's job)
    let mut b2 = [XW; 16];
    // SAFETY: a C string and a 16-unit buffer.
    assert_eq!(
        unsafe { shim::os_utf8_to_wcs(c"hello".as_ptr(), 0, ptr::null_mut(), 3) },
        5
    );
    // SAFETY: as above.
    assert_eq!(
        unsafe { shim::os_utf8_to_wcs(c"hello".as_ptr(), 0, b2.as_mut_ptr(), 16) },
        5
    );
    check_wide(&b2, &hello);
}

#[test]
fn test_utf8_to_wcs_explicit_len_keeps_nul() {
    let mut buf = [XW; 8];
    // with an explicit len an embedded NUL is converted like any byte
    assert_eq!(utf8_to_wcs(b"a\0b", None), 3);
    assert_eq!(utf8_to_wcs(b"a\0b", Some(&mut buf)), 3);
    check_wide(&buf, &[u32::from(b'a'), 0, u32::from(b'b')]);

    // len 0 stops at it
    let anb = b"a\0b\0";
    // SAFETY: a NUL-terminated string.
    assert_eq!(
        unsafe { shim::os_utf8_to_wcs(anb.as_ptr().cast(), 0, ptr::null_mut(), 0) },
        1
    );
}

#[test]
fn test_utf8_to_wcs_multibyte() {
    let utf8 = "a\u{E9}\u{20AC}\u{1F600}".as_bytes();
    let want: Vec<u32> = if WIDE_UTF32 {
        vec![0x61, 0xE9, 0x20AC, 0x1F600]
    } else {
        vec![0x61, 0xE9, 0x20AC, 0xD83D, 0xDE00]
    };
    let n = want.len();
    let mut buf = [XW; 16];

    assert_eq!(utf8_to_wcs(utf8, None), n);
    assert_eq!(utf8_to_wcs(utf8, Some(&mut buf)), n);
    check_wide(&buf, &want);

    assert_eq!(utf8_to_wcs(b"\xC3\xA9", Some(&mut buf)), 1);
    assert_eq!(buf[0], 0xE9 as WChar);
    assert_eq!(utf8_to_wcs(b"\xE2\x82\xAC", Some(&mut buf)), 1);
    assert_eq!(buf[0], 0x20AC as WChar);
    assert_eq!(utf8_to_wcs(b"\xF0\x9F\x98\x80", Some(&mut buf)), n - 3);
    check_wide(&buf, &want[3..]);

    // U+FFFF and U+10000, the edges of one and two UTF-16 units
    assert_eq!(utf8_to_wcs(b"\xEF\xBF\xBF", Some(&mut buf)), 1);
    assert_eq!(buf[0], 0xFFFF as WChar);
    if WIDE_UTF32 {
        assert_eq!(utf8_to_wcs(b"\xF0\x90\x80\x80", Some(&mut buf)), 1);
        assert_eq!(buf[0] as u32, 0x10000);
    } else {
        assert_eq!(utf8_to_wcs(b"\xF0\x90\x80\x80", Some(&mut buf)), 2);
        assert_eq!(&buf[..2], &w(&[0xD800, 0xDC00])[..]);
    }
}

#[test]
fn test_utf8_to_wcs_bom() {
    let mut buf = [XW; 8];
    let text = b"\xEF\xBB\xBFa";
    if cfg!(windows) {
        // the Windows utf8_to_wchar drops a leading UTF-8 BOM
        assert_eq!(utf8_to_wcs(text, None), 1);
        assert_eq!(utf8_to_wcs(text, Some(&mut buf)), 1);
        check_wide(&buf, &[0x61]);
    } else {
        // off Windows it is kept as U+FEFF
        assert_eq!(utf8_to_wcs(text, None), 2);
        assert_eq!(utf8_to_wcs(text, Some(&mut buf)), 2);
        check_wide(&buf, &[0xFEFF, 0x61]);
    }
}

#[test]
fn test_utf8_to_wcs_invalid() {
    let mut buf = [XW; 16];
    if cfg!(windows) {
        // MultiByteToWideChar replaces an invalid byte with U+FFFD
        let want = [0x62, 0x61, 0x64, 0x20, 0xFFFD, 0x20, 0x62, 0x79, 0x74, 0x65];
        assert_eq!(utf8_to_wcs(b"bad \xFF byte", None), 10);
        assert_eq!(utf8_to_wcs(b"bad \xFF byte", Some(&mut buf)), 10);
        check_wide(&buf, &want);
        return;
    }

    // any invalid sequence fails the whole string: 0 and an empty dst
    let bad: [&[u8]; 7] = [
        b"bad \xFF byte",
        b"\x80",
        b"\xC0\xAF",
        b"\xC3",
        b"\xE2\x82",
        b"\xE2\x82x",
        b"\xFE",
    ];
    for text in bad {
        assert_eq!(utf8_to_wcs(text, None), 0, "{text:x?}");
        buf.fill(XW);
        assert_eq!(utf8_to_wcs(text, Some(&mut buf)), 0, "{text:x?}");
        assert_eq!(buf[0], 0);
    }

    // an explicit len that cuts a sequence makes it truncated
    assert_eq!(utf8_to_wcs(&b"caf\xC3\xA9"[..4], None), 0);
    buf.fill(XW);
    assert_eq!(utf8_to_wcs(&b"caf\xC3\xA9"[..4], Some(&mut buf)), 0);
    assert_eq!(buf[0], 0);

    // Characterized, not endorsed: an encoded surrogate passes the size
    // query, then fails the conversion.
    assert_eq!(utf8_to_wcs(b"\xED\xA0\x80", None), 1);
    buf.fill(XW);
    assert_eq!(utf8_to_wcs(b"\xED\xA0\x80", Some(&mut buf)), 0);
    assert_eq!(buf[0], 0);

    // Characterized, not endorsed: overlong forms past 2 bytes, values above
    // U+10FFFF, and 5- and 6-byte sequences are accepted.
    for (text, want) in [
        (&b"\xE0\x80\xAF"[..], 0x2F),
        (b"\xF4\x90\x80\x80", 0x110000),
        (b"\xF8\x88\x80\x80\x80", 0x200000),
        (b"\xFC\x84\x80\x80\x80\x80", 0x4000000),
    ] {
        assert_eq!(utf8_to_wcs(text, Some(&mut buf)), 1);
        assert_eq!(buf[0] as u32, want);
    }
}

#[test]
fn test_utf8_to_wcs_dst_size() {
    let hello = (*b"hello").map(u32::from);
    // the buffer, then a guard
    let mut b = [XW; 9];

    // NULL str: 0, dst untouched (the shim's job)
    // SAFETY: NULL str; an 8-unit buffer.
    assert_eq!(
        unsafe { shim::os_utf8_to_wcs(ptr::null(), 0, b.as_mut_ptr(), 8) },
        0
    );
    assert_eq!(b[0], XW);
    // SAFETY: NULL str and dst.
    assert_eq!(
        unsafe { shim::os_utf8_to_wcs(ptr::null(), 0, ptr::null_mut(), 0) },
        0
    );

    // dst_size 0: 0, nothing written
    assert_eq!(utf8_to_wcs(b"hello", Some(&mut b[..0])), 0);
    assert_eq!(b[0], XW);

    // dst_size 1: room for the NUL only
    assert_eq!(utf8_to_wcs(b"hello", Some(&mut b[..1])), 0);
    assert_eq!(b[0], 0);
    assert_eq!(b[1], XW);

    // exactly the text plus its NUL
    b.fill(XW);
    b[8] = GW;
    assert_eq!(utf8_to_wcs(b"hello", Some(&mut b[..6])), 5);
    check_wide(&b, &hello);
    assert_eq!(b[6], XW);

    // Characterized, not endorsed: a dst too small for the text gives 0 and
    // an empty string, not the text cut to fit.
    b.fill(XW);
    b[8] = GW;
    assert_eq!(utf8_to_wcs(b"hello world", Some(&mut b[..4])), 0);
    assert_eq!(b[0], 0);
    assert_eq!(b[4], XW);
    assert_eq!(b[8], GW);

    b.fill(XW);
    assert_eq!(
        utf8_to_wcs("a\u{E9}\u{20AC}bcdefghij".as_bytes(), Some(&mut b[..3])),
        0
    );
    assert_eq!(b[0], 0);
    assert_eq!(b[3], XW);
}

/// New, the intentional difference: text exactly `dst_size` units long does
/// not fit with its NUL, so it gives 0 and an empty string, and nothing past
/// `dst_size` is written. C converts it in full and writes the NUL at
/// `dst[dst_size]`.
#[test]
fn utf8_to_wcs_exact_size_stays_in_dst() {
    let mut b = [XW; 9];
    b[5] = GW;
    assert_eq!(utf8_to_wcs(b"hello", Some(&mut b[..5])), 0);
    assert_eq!(b[0], 0);
    assert_eq!(b[5], GW);

    // a multibyte text too, and one wide character with dst_size 1
    let text = "a\u{E9}\u{20AC}".as_bytes();
    b.fill(XW);
    b[3] = GW;
    assert_eq!(utf8_to_wcs(text, Some(&mut b[..3])), 0);
    assert_eq!(b[0], 0);
    assert_eq!(b[3], GW);
    b.fill(XW);
    assert_eq!(utf8_to_wcs(b"a", Some(&mut b[..1])), 0);
    assert_eq!(b[0], 0);
    assert_eq!(b[1], XW);

    // the same through the shim, and as os_wcs_to_utf8 has it
    b.fill(XW);
    b[5] = GW;
    // SAFETY: a C string and a 5-unit buffer.
    assert_eq!(
        unsafe { shim::os_utf8_to_wcs(c"hello".as_ptr(), 0, b.as_mut_ptr(), 5) },
        0
    );
    assert_eq!(b[0], 0);
    assert_eq!(b[5], GW);
    let mut s = [b'x'; 6];
    s[5] = b'G';
    assert_eq!(wcs_to_utf8(&ws("hello"), Some(&mut s[..5])), 0);
    assert_eq!(s[0], 0);
    assert_eq!(s[5], b'G');
}

#[test]
fn test_utf8_to_wcs_empty() {
    let mut buf = [XW; 4];
    assert_eq!(utf8_to_wcs(b"", None), 0);
    assert_eq!(utf8_to_wcs(b"", Some(&mut buf)), 0);
    assert_eq!(buf[0], 0);
    assert_eq!(buf[1], XW);
}

// --------------------------------------------------------------------------
// os_wcs_to_utf8

#[test]
fn test_wcs_to_utf8_ascii() {
    let mut buf = [b'x'; 16];
    let hello = ws("hello");

    assert_eq!(wcs_to_utf8(&hello, None), 5);
    assert_eq!(wcs_to_utf8(&hello, Some(&mut buf)), 5);
    check_bytes(&buf, b"hello");
    assert_eq!(buf[6], b'x');

    // an explicit len, and len 0 (wcslen, the shim's job)
    assert_eq!(wcs_to_utf8(&hello[..3], None), 3);
    buf.fill(b'x');
    assert_eq!(wcs_to_utf8(&hello[..3], Some(&mut buf)), 3);
    check_bytes(&buf, b"hel");
    let hz = wsz("hello");
    // SAFETY: a NUL-terminated wide string.
    assert_eq!(
        unsafe { shim::os_wcs_to_utf8(hz.as_ptr(), 0, ptr::null_mut(), 2) },
        5
    );
    // SAFETY: as above.
    assert_eq!(
        unsafe { shim::os_wcs_to_utf8(hz.as_ptr(), 5, ptr::null_mut(), 0) },
        5
    );

    // with an explicit len an embedded NUL is converted like any unit
    let anb = wz(&[0x61, 0, 0x62]);
    buf.fill(b'x');
    assert_eq!(wcs_to_utf8(&anb[..3], Some(&mut buf)), 3);
    check_bytes(&buf, b"a\0b");
    // SAFETY: a NUL-terminated wide string.
    assert_eq!(
        unsafe { shim::os_wcs_to_utf8(anb.as_ptr(), 0, ptr::null_mut(), 0) },
        1
    );
}

#[test]
fn test_wcs_to_utf8_multibyte() {
    let wide = ws("a\u{E9}\u{20AC}\u{1F600}");
    let utf8 = "a\u{E9}\u{20AC}\u{1F600}".as_bytes();
    let mut buf = [b'x'; 16];

    assert_eq!(wcs_to_utf8(&wide, None), 10);
    assert_eq!(wcs_to_utf8(&wide, Some(&mut buf)), 10);
    check_bytes(&buf, utf8);

    for s in ["\u{E9}", "\u{20AC}", "\u{1F600}"] {
        assert_eq!(wcs_to_utf8(&ws(s), Some(&mut buf)), s.len());
        check_bytes(&buf, s.as_bytes());
    }
}

#[test]
fn test_wcs_to_utf8_invalid() {
    let lone = w(&[0x61, 0xD800, 0x62]);
    let mut buf = [b'x'; 16];

    if cfg!(windows) {
        // WideCharToMultiByte replaces a lone surrogate with U+FFFD
        assert_eq!(wcs_to_utf8(&lone, None), 5);
        assert_eq!(wcs_to_utf8(&lone, Some(&mut buf)), 5);
        check_bytes(&buf, b"a\xEF\xBF\xBDb");
        // an explicit len that splits a surrogate pair leaves a lone one
        assert_eq!(wcs_to_utf8(&w(&[0xD83D]), Some(&mut buf)), 3);
        check_bytes(&buf, b"\xEF\xBF\xBD");
        return;
    }

    // a surrogate fails the whole string: 0 and an empty dst
    assert_eq!(wcs_to_utf8(&lone, None), 0);
    assert_eq!(wcs_to_utf8(&lone, Some(&mut buf)), 0);
    assert_eq!(buf[0], 0);
    let pair = w(&[0xD83D, 0xDE00]);
    assert_eq!(wcs_to_utf8(&pair, None), 0);
    buf.fill(b'x');
    assert_eq!(wcs_to_utf8(&pair, Some(&mut buf)), 0);
    assert_eq!(buf[0], 0);

    // Characterized, not endorsed: values above U+10FFFF are encoded in 4 to
    // 6 bytes instead of being refused.
    assert_eq!(wcs_to_utf8(&w(&[0x110000]), Some(&mut buf)), 4);
    check_bytes(&buf, b"\xF4\x90\x80\x80");
    assert_eq!(wcs_to_utf8(&w(&[0x7FFF_FFFF]), Some(&mut buf)), 6);
    check_bytes(&buf, b"\xFD\xBF\xBF\xBF\xBF\xBF");
}

#[test]
fn test_wcs_to_utf8_dst_size() {
    let ae = ws("a\u{E9}");
    let hello = ws("hello");
    // the buffer, then a guard
    let mut b = [b'x'; 9];

    // SAFETY: NULL str; an 8-byte buffer.
    assert_eq!(
        unsafe { shim::os_wcs_to_utf8(ptr::null(), 0, b.as_mut_ptr().cast(), 8) },
        0
    );
    assert_eq!(b[0], b'x');
    // SAFETY: NULL str and dst.
    assert_eq!(
        unsafe { shim::os_wcs_to_utf8(ptr::null(), 0, ptr::null_mut(), 0) },
        0
    );

    assert_eq!(wcs_to_utf8(&hello, Some(&mut b[..0])), 0);
    assert_eq!(b[0], b'x');

    assert_eq!(wcs_to_utf8(&hello, Some(&mut b[..1])), 0);
    assert_eq!(b[0], 0);
    assert_eq!(b[1], b'x');

    // exactly the text plus its NUL
    b.fill(b'x');
    assert_eq!(wcs_to_utf8(&hello, Some(&mut b[..6])), 5);
    check_bytes(&b, b"hello");
    assert_eq!(b[6], b'x');

    // Characterized, not endorsed: one byte short, or a multibyte character
    // that does not fit, gives 0 and an empty string, not the text cut.
    b.fill(b'x');
    b[8] = b'G';
    assert_eq!(wcs_to_utf8(&hello, Some(&mut b[..5])), 0);
    assert_eq!(b[0], 0);
    assert_eq!(b[5], b'x');

    b.fill(b'x');
    assert_eq!(wcs_to_utf8(&ae, Some(&mut b[..3])), 0);
    assert_eq!(b[0], 0);
    assert_eq!(b[3], b'x');
    assert_eq!(wcs_to_utf8(&ae, Some(&mut b[..4])), 3);
    check_bytes(&b, b"a\xC3\xA9");

    b.fill(b'x');
    b[8] = b'G';
    assert_eq!(wcs_to_utf8(&ws("hello world"), Some(&mut b[..8])), 0);
    assert_eq!(b[0], 0);
    assert_eq!(b[8], b'G');
}

#[test]
fn test_wcs_to_utf8_empty() {
    let mut buf = [b'x'; 4];
    assert_eq!(wcs_to_utf8(&[], None), 0);
    assert_eq!(wcs_to_utf8(&[], Some(&mut buf)), 0);
    assert_eq!(buf[0], 0);
    assert_eq!(buf[1], b'x');
}

// --------------------------------------------------------------------------
// os_mbs_to_wcs and os_wcs_to_mbs (LC_CTYPE; this process stays in "C")

#[test]
fn test_mbs_to_wcs_c_locale() {
    let hello = (*b"hello").map(u32::from);
    let mut b = [XW; 9];

    assert_eq!(mbs_to_wcs(c"hello", None), 5);
    assert_eq!(mbs_to_wcs(c"hello", Some(&mut b[..8])), 5);
    check_wide(&b, &hello);
    assert_eq!(b[6], XW);

    // exact fit
    b.fill(XW);
    assert_eq!(mbs_to_wcs(c"hello", Some(&mut b[..6])), 5);
    check_wide(&b, &hello);

    // Characterized, not endorsed: len is ignored, the whole string is
    // converted (the shim drops it)
    let mut b2 = [XW; 8];
    // SAFETY: a C string and an 8-unit buffer.
    assert_eq!(
        unsafe { shim::os_mbs_to_wcs(c"hello".as_ptr(), 3, ptr::null_mut(), 0) },
        5
    );
    // SAFETY: as above.
    assert_eq!(
        unsafe { shim::os_mbs_to_wcs(c"hello".as_ptr(), 3, b2.as_mut_ptr(), 8) },
        5
    );
    check_wide(&b2, &hello);

    // too small: cut to dst_size - 1 units, unlike os_utf8_to_wcs (#263)
    b.fill(XW);
    b[8] = GW;
    assert_eq!(mbs_to_wcs(c"hello world", Some(&mut b[..8])), 7);
    check_wide(&b, &[0x68, 0x65, 0x6C, 0x6C, 0x6F, 0x20, 0x77]);
    assert_eq!(b[8], GW);

    // and exactly dst_size long: cut by one, the NUL in the buffer
    b.fill(XW);
    b[5] = GW;
    assert_eq!(mbs_to_wcs(c"hello", Some(&mut b[..5])), 4);
    check_wide(&b, &hello[..4]);
    assert_eq!(b[5], GW);

    b.fill(XW);
    assert_eq!(mbs_to_wcs(c"hello", Some(&mut b[..1])), 0);
    assert_eq!(b[0], 0);
    assert_eq!(b[1], XW);

    b.fill(XW);
    assert_eq!(mbs_to_wcs(c"hello", Some(&mut b[..0])), 0);
    assert_eq!(b[0], XW);

    // NULL and empty
    // SAFETY: NULL str; an 8-unit buffer.
    assert_eq!(
        unsafe { shim::os_mbs_to_wcs(ptr::null(), 0, b.as_mut_ptr(), 8) },
        0
    );
    assert_eq!(b[0], XW);
    // SAFETY: NULL str and dst.
    assert_eq!(
        unsafe { shim::os_mbs_to_wcs(ptr::null(), 0, ptr::null_mut(), 0) },
        0
    );
    assert_eq!(mbs_to_wcs(c"", None), 0);
    assert_eq!(mbs_to_wcs(c"", Some(&mut b[..8])), 0);
    assert_eq!(b[0], 0);
    assert_eq!(b[1], XW);
}

#[test]
fn test_wcs_to_mbs_c_locale() {
    // U+4E2D has no byte in the C locale, on every platform
    let han = wz(&[0x61, 0x4E2D]);
    let hello = wsz("hello");
    let mut b = [b'x'; 9];

    assert_eq!(wcs_to_mbs(&hello, None), 5);
    assert_eq!(wcs_to_mbs(&hello, Some(&mut b[..8])), 5);
    check_bytes(&b, b"hello");
    assert_eq!(b[6], b'x');

    b.fill(b'x');
    assert_eq!(wcs_to_mbs(&hello, Some(&mut b[..6])), 5);
    check_bytes(&b, b"hello");

    // Characterized, not endorsed: len is ignored (the shim drops it), and
    // the text ends at the first NUL; without one it is all of the slice
    let mut b2 = [b'x'; 8];
    // SAFETY: a NUL-terminated wide string and an 8-byte buffer.
    assert_eq!(
        unsafe { shim::os_wcs_to_mbs(hello.as_ptr(), 3, ptr::null_mut(), 0) },
        5
    );
    // SAFETY: as above.
    assert_eq!(
        unsafe { shim::os_wcs_to_mbs(hello.as_ptr(), 3, b2.as_mut_ptr().cast(), 8) },
        5
    );
    check_bytes(&b2, b"hello");
    assert_eq!(wcs_to_mbs(&wz(&[0x61, 0, 0x62]), None), 1);
    assert_eq!(wcs_to_mbs(&ws("hello"), None), 5);

    // too small: cut to dst_size - 1 bytes (#263)
    b.fill(b'x');
    b[8] = b'G';
    assert_eq!(wcs_to_mbs(&wsz("hello world"), Some(&mut b[..8])), 7);
    check_bytes(&b, b"hello w");
    assert_eq!(b[8], b'G');

    b.fill(b'x');
    assert_eq!(wcs_to_mbs(&hello, Some(&mut b[..1])), 0);
    assert_eq!(b[0], 0);
    assert_eq!(b[1], b'x');

    b.fill(b'x');
    assert_eq!(wcs_to_mbs(&hello, Some(&mut b[..0])), 0);
    assert_eq!(b[0], b'x');

    // unconvertible: 0 and an empty dst (#263)
    assert_eq!(wcs_to_mbs(&han, None), 0);
    b.fill(b'x');
    assert_eq!(wcs_to_mbs(&han, Some(&mut b[..8])), 0);
    assert_eq!(b[0], 0);

    b.fill(b'x');
    // SAFETY: NULL str; an 8-byte buffer.
    assert_eq!(
        unsafe { shim::os_wcs_to_mbs(ptr::null(), 0, b.as_mut_ptr().cast(), 8) },
        0
    );
    assert_eq!(b[0], b'x');
    // SAFETY: NULL str and dst.
    assert_eq!(
        unsafe { shim::os_wcs_to_mbs(ptr::null(), 0, ptr::null_mut(), 0) },
        0
    );
    assert_eq!(wcs_to_mbs(&[0], None), 0);
    assert_eq!(wcs_to_mbs(&[0], Some(&mut b[..8])), 0);
    assert_eq!(b[0], 0);
}

#[test]
fn test_mbs_utf8_locale() {
    if !common::isolated(concat!(module_path!(), "::test_mbs_utf8_locale")) {
        return;
    }
    mbs_utf8_locale_cases();
}

fn mbs_utf8_locale_cases() {
    if !set_utf8_ctype() {
        eprintln!("no UTF-8 locale installed");
        return;
    }
    let cafe = [0x63, 0x61, 0x66, 0xE9];
    let wcafe = wz(&cafe);
    let mut wb = [XW; 8];
    let mut s = [b'x'; 8];

    assert_eq!(mbs_to_wcs(c"caf\xC3\xA9", None), 4);
    assert_eq!(mbs_to_wcs(c"caf\xC3\xA9", Some(&mut wb)), 4);
    check_wide(&wb, &cafe);

    assert_eq!(wcs_to_mbs(&wcafe, None), 5);
    assert_eq!(wcs_to_mbs(&wcafe, Some(&mut s)), 5);
    check_bytes(&s, b"caf\xC3\xA9");

    // invalid UTF-8: 0 and an empty dst (#263)
    assert_eq!(mbs_to_wcs(c"bad \xFF byte", None), 0);
    wb.fill(XW);
    assert_eq!(mbs_to_wcs(c"bad \xFF byte", Some(&mut wb)), 0);
    assert_eq!(wb[0], 0);

    if !cfg!(windows) {
        // a cut never splits a character
        wb.fill(XW);
        assert_eq!(
            mbs_to_wcs(c"\xC3\xA9\xC3\xA9\xC3\xA9", Some(&mut wb[..3])),
            2
        );
        check_wide(&wb, &[0xE9, 0xE9]);

        s.fill(b'x');
        assert_eq!(wcs_to_mbs(&wcafe, Some(&mut s[..5])), 3);
        check_bytes(&s, b"caf");
        s.fill(b'x');
        assert_eq!(wcs_to_mbs(&wcafe, Some(&mut s[..6])), 5);
        check_bytes(&s, b"caf\xC3\xA9");
    }

    if WIDE_UTF32 {
        assert_eq!(mbs_to_wcs(c"\xF0\x9F\x98\x80", Some(&mut wb)), 1);
        assert_eq!(wb[0] as u32, 0x1F600);
        assert_eq!(wcs_to_mbs(&wz(&[0x1F600]), Some(&mut s)), 4);
        check_bytes(&s, b"\xF0\x9F\x98\x80");
    }

    // the Vec forms platform_file uses, over the _ptr compositions
    assert_eq!(utf8_to_mbs(b"caf\xC3\xA9"), b"caf\xC3\xA9");
    assert_eq!(mbs_to_utf8(c"caf\xC3\xA9"), b"caf\xC3\xA9");
    assert_eq!(mbs_to_utf8(c"bad \xFF byte"), b"");
}

// --------------------------------------------------------------------------
// the _ptr forms

/// The safe core's `_ptr` forms: one allocation of the size query plus one,
/// then the conversion into it.
#[test]
fn ptr_forms_allocate_size_plus_one() {
    let mut wbuf = Vec::new();
    assert_eq!(utf8_to_wcs_ptr(b"caf\xC3\xA9", vec_alloc(&mut wbuf, XW)), 4);
    assert_eq!(wbuf.len(), 5);
    check_wide(&wbuf, &[0x63, 0x61, 0x66, 0xE9]);

    let mut sbuf = Vec::new();
    assert_eq!(
        wcs_to_utf8_ptr(&ws("caf\u{E9}"), vec_alloc(&mut sbuf, b'x')),
        5
    );
    assert_eq!(sbuf.len(), 6);
    check_bytes(&sbuf, b"caf\xC3\xA9");

    let mut wbuf = Vec::new();
    assert_eq!(mbs_to_wcs_ptr(c"hello", vec_alloc(&mut wbuf, XW)), 5);
    assert_eq!(wbuf.len(), 6);

    let mut sbuf = Vec::new();
    assert_eq!(wcs_to_mbs_ptr(&wsz("hello"), vec_alloc(&mut sbuf, b'x')), 5);
    assert_eq!(sbuf.len(), 6);
    check_bytes(&sbuf, b"hello");

    // invalid input off Windows: the size query's buffer, then an empty string
    if !cfg!(windows) {
        let mut wbuf = Vec::new();
        assert_eq!(
            utf8_to_wcs_ptr(b"\xED\xA0\x80", vec_alloc(&mut wbuf, XW)),
            0
        );
        assert_eq!(wbuf.len(), 2);
        assert_eq!(wbuf[0], 0);
    }

    // the C locale cannot hold U+4E2D
    assert_eq!(utf8_to_mbs(b"a\xE4\xB8\xAD"), b"");
    assert_eq!(utf8_to_mbs(&b"hello"[..3]), b"hel");
    assert_eq!(mbs_to_utf8(c"hello"), b"hello");
}

#[test]
fn test_utf8_wcs_ptr() {
    if !common::isolated(concat!(module_path!(), "::test_utf8_wcs_ptr")) {
        return;
    }
    let cafe = [0x63, 0x61, 0x66, 0xE9];
    let wcafe = wz(&cafe);
    // SAFETY: the bmem shim's counter.
    let allocs = unsafe { bnum_allocs() };
    let count = || unsafe { bnum_allocs() } - allocs;
    let mut wp: *mut WChar = ptr::dangling_mut();
    let mut sp: *mut c_char = ptr::dangling_mut();

    // SAFETY (this test's shim calls): C strings, writable `*pstr`.
    unsafe {
        // one allocation, freed with bfree
        assert_eq!(
            shim::os_utf8_to_wcs_ptr(c"caf\xC3\xA9".as_ptr(), 0, &mut wp),
            4
        );
        assert_eq!(count(), 1);
        assert_eq!(take_wide(wp).unwrap(), w(&cafe));
        assert_eq!(count(), 0);

        assert_eq!(
            shim::os_utf8_to_wcs_ptr(c"caf\xC3\xA9".as_ptr(), 3, &mut wp),
            3
        );
        assert_eq!(take_wide(wp).unwrap(), w(&cafe[..3]));

        assert_eq!(shim::os_wcs_to_utf8_ptr(wcafe.as_ptr(), 0, &mut sp), 5);
        assert_eq!(count(), 1);
        assert_eq!(take_bytes(sp).unwrap(), b"caf\xC3\xA9");
        assert_eq!(count(), 0);

        assert_eq!(shim::os_wcs_to_utf8_ptr(wcafe.as_ptr(), 3, &mut sp), 3);
        assert_eq!(take_bytes(sp).unwrap(), b"caf");

        // empty: an allocated empty string
        assert_eq!(shim::os_utf8_to_wcs_ptr(c"".as_ptr(), 0, &mut wp), 0);
        assert_eq!(take_wide(wp).unwrap(), []);
        let empty: [WChar; 1] = [0];
        assert_eq!(shim::os_wcs_to_utf8_ptr(empty.as_ptr(), 0, &mut sp), 0);
        assert_eq!(take_bytes(sp).unwrap(), b"");

        // NULL: *pstr is set to NULL, nothing allocated
        wp = ptr::dangling_mut();
        sp = ptr::dangling_mut();
        assert_eq!(shim::os_utf8_to_wcs_ptr(ptr::null(), 0, &mut wp), 0);
        assert!(wp.is_null());
        assert_eq!(shim::os_wcs_to_utf8_ptr(ptr::null(), 0, &mut sp), 0);
        assert!(sp.is_null());
        assert_eq!(count(), 0);

        if !cfg!(windows) {
            // invalid input: 0 and an allocated empty string
            let lone = wz(&[0xD800]);
            assert_eq!(
                shim::os_utf8_to_wcs_ptr(c"bad \xFF byte".as_ptr(), 0, &mut wp),
                0
            );
            assert_eq!(take_wide(wp).unwrap(), []);
            assert_eq!(
                shim::os_utf8_to_wcs_ptr(c"\xED\xA0\x80".as_ptr(), 0, &mut wp),
                0
            );
            assert_eq!(take_wide(wp).unwrap(), []);
            assert_eq!(shim::os_wcs_to_utf8_ptr(lone.as_ptr(), 0, &mut sp), 0);
            assert_eq!(take_bytes(sp).unwrap(), b"");
            assert_eq!(count(), 0);
        }
    }
}

#[test]
fn test_mbs_wcs_ptr() {
    if !common::isolated(concat!(module_path!(), "::test_mbs_wcs_ptr")) {
        return;
    }
    let hello = (*b"hello").map(u32::from);
    let han = wz(&[0x61, 0x4E2D]);
    let whello = wsz("hello");
    // SAFETY: the bmem shim's counter.
    let allocs = unsafe { bnum_allocs() };
    let count = || unsafe { bnum_allocs() } - allocs;
    let mut wp: *mut WChar = ptr::dangling_mut();
    let mut sp: *mut c_char = ptr::dangling_mut();

    // SAFETY (this test's shim calls): C strings, writable `*pstr`.
    unsafe {
        assert_eq!(shim::os_mbs_to_wcs_ptr(c"hello".as_ptr(), 0, &mut wp), 5);
        assert_eq!(count(), 1);
        assert_eq!(take_wide(wp).unwrap(), w(&hello));

        // Characterized, not endorsed: len is ignored
        assert_eq!(shim::os_mbs_to_wcs_ptr(c"hello".as_ptr(), 3, &mut wp), 5);
        assert_eq!(take_wide(wp).unwrap(), w(&hello));

        assert_eq!(shim::os_wcs_to_mbs_ptr(whello.as_ptr(), 0, &mut sp), 5);
        assert_eq!(count(), 1);
        assert_eq!(take_bytes(sp).unwrap(), b"hello");
        assert_eq!(shim::os_wcs_to_mbs_ptr(whello.as_ptr(), 3, &mut sp), 5);
        assert_eq!(take_bytes(sp).unwrap(), b"hello");

        assert_eq!(shim::os_mbs_to_wcs_ptr(c"".as_ptr(), 0, &mut wp), 0);
        assert_eq!(take_wide(wp).unwrap(), []);
        let empty: [WChar; 1] = [0];
        assert_eq!(shim::os_wcs_to_mbs_ptr(empty.as_ptr(), 0, &mut sp), 0);
        assert_eq!(take_bytes(sp).unwrap(), b"");

        // unconvertible: 0 and an allocated empty string (#263)
        assert_eq!(shim::os_wcs_to_mbs_ptr(han.as_ptr(), 0, &mut sp), 0);
        assert_eq!(take_bytes(sp).unwrap(), b"");

        wp = ptr::dangling_mut();
        sp = ptr::dangling_mut();
        assert_eq!(shim::os_mbs_to_wcs_ptr(ptr::null(), 0, &mut wp), 0);
        assert!(wp.is_null());
        assert_eq!(shim::os_wcs_to_mbs_ptr(ptr::null(), 0, &mut sp), 0);
        assert!(sp.is_null());
        assert_eq!(count(), 0);
    }
}

#[test]
fn test_utf8_mbs_ptr() {
    if !common::isolated(concat!(module_path!(), "::test_utf8_mbs_ptr")) {
        return;
    }
    // SAFETY: the bmem shim's counter.
    let allocs = unsafe { bnum_allocs() };
    let count = || unsafe { bnum_allocs() } - allocs;
    let mut sp: *mut c_char = ptr::dangling_mut();

    // SAFETY (this test's shim calls): C strings, writable `*pstr`.
    unsafe {
        // the wide buffer in between is freed: one allocation left
        assert_eq!(shim::os_utf8_to_mbs_ptr(c"hello".as_ptr(), 0, &mut sp), 5);
        assert_eq!(count(), 1);
        assert_eq!(take_bytes(sp).unwrap(), b"hello");
        assert_eq!(count(), 0);

        sp = ptr::dangling_mut();
        assert_eq!(shim::os_mbs_to_utf8_ptr(c"hello".as_ptr(), 0, &mut sp), 5);
        assert_eq!(count(), 1);
        assert_eq!(take_bytes(sp).unwrap(), b"hello");
        assert_eq!(count(), 0);

        // os_utf8_to_mbs_ptr honors len; os_mbs_to_utf8_ptr ignores it
        // (characterized, not endorsed)
        assert_eq!(shim::os_utf8_to_mbs_ptr(c"hello".as_ptr(), 3, &mut sp), 3);
        assert_eq!(take_bytes(sp).unwrap(), b"hel");
        assert_eq!(shim::os_mbs_to_utf8_ptr(c"hello".as_ptr(), 3, &mut sp), 5);
        assert_eq!(take_bytes(sp).unwrap(), b"hello");

        // empty
        assert_eq!(shim::os_utf8_to_mbs_ptr(c"".as_ptr(), 0, &mut sp), 0);
        assert_eq!(take_bytes(sp).unwrap(), b"");
        assert_eq!(shim::os_mbs_to_utf8_ptr(c"".as_ptr(), 0, &mut sp), 0);
        assert_eq!(take_bytes(sp).unwrap(), b"");

        // text the C locale cannot hold: 0 and an allocated empty string
        assert_eq!(
            shim::os_utf8_to_mbs_ptr(c"a\xE4\xB8\xAD".as_ptr(), 0, &mut sp),
            0
        );
        assert_eq!(take_bytes(sp).unwrap(), b"");

        // NULL
        sp = ptr::dangling_mut();
        assert_eq!(shim::os_utf8_to_mbs_ptr(ptr::null(), 0, &mut sp), 0);
        assert!(sp.is_null());
        sp = ptr::dangling_mut();
        assert_eq!(shim::os_mbs_to_utf8_ptr(ptr::null(), 0, &mut sp), 0);
        assert!(sp.is_null());
        assert_eq!(count(), 0);

        if set_utf8_ctype() {
            assert_eq!(
                shim::os_utf8_to_mbs_ptr(c"caf\xC3\xA9".as_ptr(), 0, &mut sp),
                5
            );
            assert_eq!(take_bytes(sp).unwrap(), b"caf\xC3\xA9");
            assert_eq!(
                shim::os_mbs_to_utf8_ptr(c"caf\xC3\xA9".as_ptr(), 0, &mut sp),
                5
            );
            assert_eq!(take_bytes(sp).unwrap(), b"caf\xC3\xA9");

            // invalid UTF-8 on the mbs side (#263)
            assert_eq!(
                shim::os_mbs_to_utf8_ptr(c"bad \xFF byte".as_ptr(), 0, &mut sp),
                0
            );
            assert_eq!(take_bytes(sp).unwrap(), b"");
            if !cfg!(windows) {
                assert_eq!(
                    shim::os_utf8_to_mbs_ptr(c"bad \xFF byte".as_ptr(), 0, &mut sp),
                    0
                );
                assert_eq!(take_bytes(sp).unwrap(), b"");
            }
            assert_eq!(count(), 0);
        } else {
            eprintln!("no UTF-8 locale: UTF-8 locale cases skipped");
        }
    }
}
