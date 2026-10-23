//! Tier 3: the Rust C ABI shims for the string conversions behave exactly
//! like the original `libobs/util/platform-conversions.c`, compiled as
//! `oracle_*` (`obs_c_oracle::platform_conversions`).
//!
//! Each case calls the Rust shim and the C with the same input and compares
//! the return values and every element of the destination buffers, which
//! start out filled with the same sentinel and are [`PAD`] elements longer
//! than the `dst_size` both sides are given. The inputs are arbitrary bytes
//! and valid and invalid UTF-8 (overlong, surrogate, above U+10FFFF, 5- and
//! 6-byte, truncated, a BOM, embedded NULs), arbitrary wide units (lone and
//! paired surrogates, values above U+10FFFF, negative `wchar_t`), `len` 0 and
//! every explicit `len`, a NULL `dst` (a size query, with a random
//! `dst_size` it must ignore), and every `dst_size` around the length the
//! text needs. Both sides use the host C library's `mbstowcs` / `wcstombs`
//! in the same `LC_CTYPE`: the C locale here, a UTF-8 one in a child of the
//! test binary. The `_ptr` forms are compared on their results and on the
//! allocations they leave (`bnum_allocs`, in a child).
//!
//! Intentional difference, excluded from the comparison exactly: C writes
//! one element past the buffer; Rust doesn't. `os_utf8_to_wcs` in C gives
//! `utf8_to_wchar` a capacity of `dst_size` (`os_wcs_to_utf8` gives
//! `dst_size - 1`), so for text exactly `dst_size` wide characters long it
//! returns `dst_size` and writes its NUL at `dst[dst_size]`. The Rust gives
//! `dst_size - 1`, as `os_wcs_to_utf8` does, so that text does not fit: 0 and
//! an empty string. The same capacity also lets the C's failed conversion of
//! longer text leave one more unit of partial text after the NUL, at
//! `dst[dst_size - 1]`; both still return 0 and an empty string. So for every
//! `dst_size` of 2 or more, the C must match `utf8_to_wchar` (the original,
//! `oracle_utf8_to_wchar`) into `dst_size` units plus the NUL, and the Rust
//! the same into `dst_size - 1` units; the two must be identical except
//! exactly where those differ ([`Outcome::ExactSize`],
//! [`Outcome::PartialTail`]). The Rust writes nothing past `dst_size` for any
//! input.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use core::ffi::{CStr, c_char, c_long};
use core::ptr;

use obs_c_oracle::bmem::{oracle_bfree, oracle_bnum_allocs};
use obs_c_oracle::platform_conversions::{self as c, WChar as CWChar};
use obs_c_oracle::utf8::oracle_utf8_to_wchar;
use obs_util::ffi::bmem::{bfree, bnum_allocs};
use obs_util::ffi::platform_conversions as rs;
use obs_util::platform_conversions::WChar;
use proptest::prelude::*;

use crate::common;

/// Extra elements after `dst_size` in each buffer.
const PAD: usize = 2;
/// The fill of every buffer before a call.
const SENT_W: u32 = 0xBEEF;
const SENT_B: u8 = 0xA5;
const WIDE_UTF32: bool = size_of::<WChar>() == 4;

fn rw(units: &[u32]) -> Vec<WChar> {
    units.iter().map(|&u| u as WChar).collect()
}

fn cw(units: &[u32]) -> Vec<CWChar> {
    units.iter().map(|&u| u as CWChar).collect()
}

fn rw_units(buf: &[WChar]) -> Vec<u32> {
    buf.iter().map(|&u| u as u32).collect()
}

fn cw_units(buf: &[CWChar]) -> Vec<u32> {
    buf.iter().map(|&u| u as u32).collect()
}

/// `text` with a NUL after it, for `len` 0 (and harmless otherwise).
fn nul(text: &[u8]) -> Vec<u8> {
    let mut z = text.to_vec();
    z.push(0);
    z
}

fn nulw(units: &[u32]) -> Vec<u32> {
    let mut z = units.to_vec();
    z.push(0);
    z
}

/// Which way an `os_utf8_to_wcs` input came out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    /// Both sides gave the same result and buffer.
    Same,
    /// Excluded: text exactly `dst_size` long; the C wrote `dst[dst_size]`.
    ExactSize,
    /// Excluded: text that does not fit, where the C's failed conversion
    /// left one more unit of partial text, at `dst[dst_size - 1]`.
    PartialTail,
}

/// What `os_utf8_to_wcs` gives when it hands `utf8_to_wchar` a capacity of
/// `cap` units of an `n`-unit `dst` (`n >= 2`): the result, and the padded
/// buffer with the NUL at the result. The original `utf8_to_wchar`
/// (`oracle_utf8_to_wchar`) does the converting.
fn with_capacity(s: *const c_char, len: usize, n: usize, cap: usize) -> (usize, Vec<u32>) {
    let mut buf = cw(&vec![SENT_W; n + PAD]);
    // SAFETY: `s` is readable for `len` bytes and NUL-terminated; `buf` holds
    // `n + PAD >= cap` units.
    let got = unsafe { oracle_utf8_to_wchar(s, len, buf.as_mut_ptr(), cap, 0) };
    buf[got] = 0;
    (got, cw_units(&buf))
}

/// `os_utf8_to_wcs` on both sides, as the module docs say: the same result
/// and buffer, except for exactly the inputs where the capacity the C passes
/// (`dst_size`) and the one the Rust passes (`dst_size - 1`) give different
/// conversions.
fn utf8_to_wcs_both(text: &[u8], len: usize, dst: Option<usize>, junk: usize) -> Outcome {
    let z = nul(text);
    let s = z.as_ptr().cast::<c_char>();
    let Some(n) = dst else {
        // SAFETY: `s` is readable for `len` bytes and NUL-terminated.
        let (r, k) = unsafe {
            (
                rs::os_utf8_to_wcs(s, len, ptr::null_mut(), junk),
                c::oracle_os_utf8_to_wcs(s, len, ptr::null_mut(), junk),
            )
        };
        assert_eq!(r, k, "size query of {text:x?}, len {len}");
        return Outcome::Same;
    };
    let mut rb = rw(&vec![SENT_W; n + PAD]);
    let mut cb = cw(&vec![SENT_W; n + PAD]);
    // SAFETY: both buffers hold `n + PAD` units; the C may write `n + 1`.
    let (r, k) = unsafe {
        (
            rs::os_utf8_to_wcs(s, len, rb.as_mut_ptr(), n),
            c::oracle_os_utf8_to_wcs(s, len, cb.as_mut_ptr(), n),
        )
    };
    let (rb, cb) = (rw_units(&rb), cw_units(&cb));
    let at = format!("{text:x?}, len {len}, dst_size {n}");
    assert!(
        rb[n..].iter().all(|&u| u == SENT_W),
        "Rust wrote past dst_size: {at}"
    );
    if n < 2 {
        assert_eq!((r, &rb), (k, &cb), "{at}");
        return Outcome::Same;
    }
    // The C is os_utf8_to_wcs with capacity n, the Rust with n - 1.
    assert_eq!((k, cb.clone()), with_capacity(s, len, n, n), "C: {at}");
    assert_eq!(
        (r, rb.clone()),
        with_capacity(s, len, n, n - 1),
        "Rust: {at}"
    );
    if (r, &rb) == (k, &cb) {
        Outcome::Same
    } else if k == n {
        assert_eq!(cb[n], 0, "{at}");
        assert_eq!((r, rb[0]), (0, 0), "{at}");
        Outcome::ExactSize
    } else {
        // Both failed; only the leftover unit after the NUL differs.
        assert_eq!((r, k), (0, 0), "{at}");
        assert_eq!(rb[..n - 1], cb[..n - 1], "{at}");
        assert_eq!(rb[n - 1], SENT_W, "{at}");
        Outcome::PartialTail
    }
}

/// `os_wcs_to_utf8` on both sides.
fn wcs_to_utf8_both(units: &[u32], len: usize, dst: Option<usize>, junk: usize) {
    let (rz, cz) = (rw(&nulw(units)), cw(&nulw(units)));
    let Some(n) = dst else {
        // SAFETY: readable for `len` units and NUL-terminated.
        let (r, k) = unsafe {
            (
                rs::os_wcs_to_utf8(rz.as_ptr(), len, ptr::null_mut(), junk),
                c::oracle_os_wcs_to_utf8(cz.as_ptr(), len, ptr::null_mut(), junk),
            )
        };
        assert_eq!(r, k, "size query of {units:x?}, len {len}");
        return;
    };
    let mut rb = vec![SENT_B; n + PAD];
    let mut cb = vec![SENT_B; n + PAD];
    // SAFETY: both buffers hold `n + PAD` bytes.
    let (r, k) = unsafe {
        (
            rs::os_wcs_to_utf8(rz.as_ptr(), len, rb.as_mut_ptr().cast(), n),
            c::oracle_os_wcs_to_utf8(cz.as_ptr(), len, cb.as_mut_ptr().cast(), n),
        )
    };
    assert_eq!(r, k, "{units:x?}, len {len}, dst_size {n}");
    assert_eq!(rb, cb, "{units:x?}, len {len}, dst_size {n}");
}

/// `os_mbs_to_wcs` on both sides; `text` has no NUL.
fn mbs_to_wcs_both(text: &[u8], len: usize, dst: Option<usize>, junk: usize) {
    let z = nul(text);
    let s = z.as_ptr().cast::<c_char>();
    let Some(n) = dst else {
        // SAFETY: NUL-terminated.
        let (r, k) = unsafe {
            (
                rs::os_mbs_to_wcs(s, len, ptr::null_mut(), junk),
                c::oracle_os_mbs_to_wcs(s, len, ptr::null_mut(), junk),
            )
        };
        assert_eq!(r, k, "size query of {text:x?}");
        return;
    };
    let mut rb = rw(&vec![SENT_W; n + PAD]);
    let mut cb = cw(&vec![SENT_W; n + PAD]);
    // SAFETY: both buffers hold `n + PAD` units.
    let (r, k) = unsafe {
        (
            rs::os_mbs_to_wcs(s, len, rb.as_mut_ptr(), n),
            c::oracle_os_mbs_to_wcs(s, len, cb.as_mut_ptr(), n),
        )
    };
    assert_eq!(r, k, "{text:x?}, dst_size {n}");
    assert_eq!(rw_units(&rb), cw_units(&cb), "{text:x?}, dst_size {n}");
}

/// `os_wcs_to_mbs` on both sides; `units` has no NUL.
fn wcs_to_mbs_both(units: &[u32], len: usize, dst: Option<usize>, junk: usize) {
    let (rz, cz) = (rw(&nulw(units)), cw(&nulw(units)));
    let Some(n) = dst else {
        // SAFETY: NUL-terminated.
        let (r, k) = unsafe {
            (
                rs::os_wcs_to_mbs(rz.as_ptr(), len, ptr::null_mut(), junk),
                c::oracle_os_wcs_to_mbs(cz.as_ptr(), len, ptr::null_mut(), junk),
            )
        };
        assert_eq!(r, k, "size query of {units:x?}");
        return;
    };
    let mut rb = vec![SENT_B; n + PAD];
    let mut cb = vec![SENT_B; n + PAD];
    // SAFETY: both buffers hold `n + PAD` bytes.
    let (r, k) = unsafe {
        (
            rs::os_wcs_to_mbs(rz.as_ptr(), len, rb.as_mut_ptr().cast(), n),
            c::oracle_os_wcs_to_mbs(cz.as_ptr(), len, cb.as_mut_ptr().cast(), n),
        )
    };
    assert_eq!(r, k, "{units:x?}, dst_size {n}");
    assert_eq!(rb, cb, "{units:x?}, dst_size {n}");
}

/// A `_ptr` result: its `ret` elements and the NUL after them, then freed
/// with that side's `bfree`. `None` for NULL.
///
/// # Safety
///
/// `p` is NULL or a `bmalloc`ed buffer of at least `ret + 1` elements.
unsafe fn take_rw(p: *mut WChar, ret: usize) -> Option<Vec<u32>> {
    // SAFETY: the caller's contract.
    (!p.is_null()).then(|| unsafe {
        let out = (0..=ret).map(|i| *p.add(i) as u32).collect();
        bfree(p.cast());
        out
    })
}

/// [`take_rw`] for the C side.
///
/// # Safety
///
/// As [`take_rw`].
unsafe fn take_cw(p: *mut CWChar, ret: usize) -> Option<Vec<u32>> {
    // SAFETY: the caller's contract.
    (!p.is_null()).then(|| unsafe {
        let out = (0..=ret).map(|i| *p.add(i) as u32).collect();
        oracle_bfree(p.cast());
        out
    })
}

/// [`take_rw`] for bytes; `c` picks the oracle's `bfree`.
///
/// # Safety
///
/// As [`take_rw`].
unsafe fn take_b(p: *mut c_char, ret: usize, c: bool) -> Option<Vec<u8>> {
    // SAFETY: the caller's contract.
    (!p.is_null()).then(|| unsafe {
        let out = core::slice::from_raw_parts(p.cast::<u8>(), ret + 1).to_vec();
        if c {
            oracle_bfree(p.cast());
        } else {
            bfree(p.cast());
        }
        out
    })
}

/// The `_ptr` forms on both sides: results and returned strings. `text` is
/// the byte input, `units` the wide one; NULL when `null`.
fn ptr_forms_both(text: &[u8], units: &[u32], len: usize, wlen: usize, null: bool) {
    let z = nul(text);
    let s: *const c_char = if null { ptr::null() } else { z.as_ptr().cast() };
    let (rz, cz) = (rw(&nulw(units)), cw(&nulw(units)));
    let (rws, cws) = if null {
        (ptr::null(), ptr::null())
    } else {
        (rz.as_ptr(), cz.as_ptr())
    };
    // SAFETY (all calls below): inputs NULL or NUL-terminated and readable
    // for their `len`; `*pstr` writable.
    unsafe {
        let (mut pr, mut pc): (*mut WChar, *mut CWChar) =
            (ptr::dangling_mut(), ptr::dangling_mut());
        let (r, k) = (
            rs::os_utf8_to_wcs_ptr(s, len, &mut pr),
            c::oracle_os_utf8_to_wcs_ptr(s, len, &mut pc),
        );
        assert_eq!(r, k, "utf8_to_wcs_ptr {text:x?}, len {len}");
        assert_eq!(take_rw(pr, r), take_cw(pc, k), "utf8_to_wcs_ptr {text:x?}");

        let (r, k) = (
            rs::os_mbs_to_wcs_ptr(s, len, &mut pr),
            c::oracle_os_mbs_to_wcs_ptr(s, len, &mut pc),
        );
        assert_eq!(r, k, "mbs_to_wcs_ptr {text:x?}");
        assert_eq!(take_rw(pr, r), take_cw(pc, k), "mbs_to_wcs_ptr {text:x?}");

        let (mut br, mut bc): (*mut c_char, *mut c_char) =
            (ptr::dangling_mut(), ptr::dangling_mut());
        for (name, f_r, f_c) in [
            (
                "wcs_to_utf8_ptr",
                rs::os_wcs_to_utf8_ptr
                    as unsafe extern "C" fn(*const WChar, usize, *mut *mut c_char) -> usize,
                c::oracle_os_wcs_to_utf8_ptr
                    as unsafe extern "C" fn(*const CWChar, usize, *mut *mut c_char) -> usize,
            ),
            (
                "wcs_to_mbs_ptr",
                rs::os_wcs_to_mbs_ptr,
                c::oracle_os_wcs_to_mbs_ptr,
            ),
        ] {
            let (r, k) = (f_r(rws, wlen, &mut br), f_c(cws, wlen, &mut bc));
            assert_eq!(r, k, "{name} {units:x?}, len {wlen}");
            assert_eq!(
                take_b(br, r, false),
                take_b(bc, k, true),
                "{name} {units:x?}"
            );
        }
        for (name, f_r, f_c) in [
            (
                "utf8_to_mbs_ptr",
                rs::os_utf8_to_mbs_ptr
                    as unsafe extern "C" fn(*const c_char, usize, *mut *mut c_char) -> usize,
                c::oracle_os_utf8_to_mbs_ptr
                    as unsafe extern "C" fn(*const c_char, usize, *mut *mut c_char) -> usize,
            ),
            (
                "mbs_to_utf8_ptr",
                rs::os_mbs_to_utf8_ptr,
                c::oracle_os_mbs_to_utf8_ptr,
            ),
        ] {
            let (r, k) = (f_r(s, len, &mut br), f_c(s, len, &mut bc));
            assert_eq!(r, k, "{name} {text:x?}, len {len}");
            assert_eq!(
                take_b(br, r, false),
                take_b(bc, k, true),
                "{name} {text:x?}"
            );
        }
    }
}

// --------------------------------------------------------------------------
// inputs

/// Pieces of UTF-8 text, valid and not.
const PIECES: &[&[u8]] = &[
    b"a",
    b"hello",
    b" ",
    b"\0",
    b"\xC3\xA9",
    b"\xE2\x82\xAC",
    b"\xF0\x9F\x98\x80",
    b"\xEF\xBF\xBF",
    b"\xF0\x90\x80\x80",
    b"\xEF\xBB\xBF",
    b"\xED\xA0\x80",
    b"\xED\xBF\xBF",
    b"\xC0\xAF",
    b"\xC1\xBF",
    b"\xE0\x80\xAF",
    b"\xF0\x80\x80\xAF",
    b"\xF4\x8F\xBF\xBF",
    b"\xF4\x90\x80\x80",
    b"\xF8\x88\x80\x80\x80",
    b"\xFC\x84\x80\x80\x80\x80",
    b"\x80",
    b"\xBF",
    b"\xC3",
    b"\xE2\x82",
    b"\xF0\x9F\x98",
    b"\xF5",
    b"\xFE",
    b"\xFF",
];

fn utf8_text() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        prop::collection::vec(any::<u8>(), 0..24),
        prop::collection::vec(prop::sample::select(PIECES), 0..8).prop_map(|p| p.concat()),
        ".{0,12}".prop_map(String::into_bytes),
    ]
}

/// Wide units: ASCII, BMP, surrogates, supplementary planes, above U+10FFFF,
/// negative `wchar_t` (as `u32`) and NUL.
fn wide_unit() -> impl Strategy<Value = u32> {
    prop_oneof![
        0x20u32..0x7F,
        Just(0u32),
        0x80u32..0xD800,
        0xD800u32..0xE000,
        0xE000u32..0x10000,
        0x10000u32..0x110000,
        0x110000u32..=0x7FFF_FFFF,
        0x8000_0000u32..=u32::MAX,
    ]
    .prop_map(|u| if WIDE_UTF32 { u } else { u & 0xFFFF })
}

fn wide_text() -> impl Strategy<Value = Vec<u32>> {
    prop::collection::vec(wide_unit(), 0..16)
}

/// `len`: 0 (up to the NUL) or an explicit length.
fn lens(n: usize) -> Vec<usize> {
    (0..=n).collect()
}

/// Every `dst_size` from 0 to two past `need` (the text plus its NUL), and
/// NULL.
fn sizes(need: usize) -> Vec<Option<usize>> {
    std::iter::once(None)
        .chain((0..=need + 2).map(Some))
        .collect()
}

/// The C size query (for choosing `dst_size`s).
fn utf8_need(text: &[u8], len: usize) -> usize {
    let z = nul(text);
    // SAFETY: NUL-terminated and readable for `len` bytes.
    unsafe { c::oracle_os_utf8_to_wcs(z.as_ptr().cast(), len, ptr::null_mut(), 0) + 1 }
}

/// Every `len` and `dst_size` of `text`; counts the outcomes.
fn utf8_cases(text: &[u8], junk: usize) -> [usize; 3] {
    let mut seen = [0; 3];
    for len in lens(text.len()) {
        for dst in sizes(utf8_need(text, len)) {
            seen[utf8_to_wcs_both(text, len, dst, junk) as usize] += 1;
        }
    }
    seen
}

fn wcs_utf8_cases(units: &[u32], junk: usize) {
    for len in lens(units.len()) {
        let cz = cw(&nulw(units));
        // SAFETY: NUL-terminated and readable for `len` units.
        let need = unsafe { c::oracle_os_wcs_to_utf8(cz.as_ptr(), len, ptr::null_mut(), 0) } + 1;
        for dst in sizes(need.max(units.len() * 6 + 1)) {
            wcs_to_utf8_both(units, len, dst, junk);
        }
    }
}

/// The mbs side reads up to the NUL whatever `len` says.
fn mbs_cases(text: &[u8], units: &[u32], junk: usize) {
    let text: Vec<u8> = text.iter().copied().take_while(|&b| b != 0).collect();
    let units: Vec<u32> = units.iter().copied().take_while(|&u| u != 0).collect();
    for dst in sizes(text.len() + 1) {
        mbs_to_wcs_both(&text, junk % 7, dst, junk);
    }
    for dst in sizes(units.len() * 6 + 1) {
        wcs_to_mbs_both(&units, junk % 7, dst, junk);
    }
}

// --------------------------------------------------------------------------
// fixed cases

/// The cmocka inputs and the exact-size boundary, every `len` and `dst_size`.
#[test]
fn characterization_inputs_match_oracle() {
    let mut seen = [0; 3];
    for text in PIECES.iter().map(|p| p.to_vec()).chain([
        b"hello world".to_vec(),
        "a\u{E9}\u{20AC}\u{1F600}".as_bytes().to_vec(),
        b"bad \xFF byte".to_vec(),
        b"caf\xC3\xA9".to_vec(),
        b"a\0b".to_vec(),
        Vec::new(),
    ]) {
        for (total, n) in seen.iter_mut().zip(utf8_cases(&text, 3)) {
            *total += n;
        }
        ptr_forms_both(&text, &[], text.len().min(3), 0, false);
    }
    // both kinds of excluded input are reached
    let [same, exact, tail] = seen;
    assert!(same >= 500 && exact >= 10 && tail >= 10, "{seen:?}");

    for units in [
        vec![0x68, 0x65, 0x6C, 0x6C, 0x6F],
        vec![0x61, 0xE9, 0x20AC, 0x1F600],
        vec![0x61, 0xD83D, 0xDE00],
        vec![0x61, 0xD800, 0x62],
        vec![0x110000],
        vec![0x7FFF_FFFF],
        vec![0x61, 0, 0x62],
        vec![0x61, 0x4E2D],
        vec![],
    ] {
        let units: Vec<u32> = units
            .into_iter()
            .map(|u| if WIDE_UTF32 { u } else { u & 0xFFFF })
            .collect();
        wcs_utf8_cases(&units, 9);
        mbs_cases(b"hello", &units, 5);
        ptr_forms_both(b"", &units, 0, units.len().min(2), false);
    }
    ptr_forms_both(b"x", &[0x78], 0, 0, true);
}

/// The difference, spelled out. Exactly `dst_size` long: C returns
/// `dst_size` and writes its NUL at `dst[dst_size]`, Rust returns 0 and an
/// empty string. One unit longer than fits: both return 0 and an empty
/// string, and only the C's leftover partial text reaches `dst[dst_size - 1]`.
/// With room for the NUL, or with a `dst_size` of 1, they agree.
#[test]
fn capacity_is_the_only_difference() {
    for (text, n) in [
        (&b"hello"[..], 5),
        (b"a", 1),
        (b"ab", 2),
        ("a\u{E9}\u{20AC}".as_bytes(), 3),
        (b"\xF0\x9F\x98\x80", if WIDE_UTF32 { 1 } else { 2 }),
    ] {
        let exact = if n >= 2 {
            Outcome::ExactSize
        } else {
            Outcome::Same
        };
        assert_eq!(utf8_to_wcs_both(text, 0, Some(n), 0), exact, "{text:x?}");
        assert_eq!(utf8_to_wcs_both(text, 0, Some(n + 1), 0), Outcome::Same);
        let short = if n > 2 {
            Outcome::PartialTail
        } else {
            Outcome::Same
        };
        assert_eq!(
            utf8_to_wcs_both(text, 0, Some(n - 1), 0),
            short,
            "{text:x?}"
        );
    }
    assert_eq!(
        utf8_to_wcs_both(b"hello world", 0, Some(4), 0),
        Outcome::PartialTail
    );
    // Off Windows an invalid sequence stops both before the capacity
    // matters. On Windows MultiByteToWideChar turns it into U+FFFD instead,
    // so the text is 6 units and runs into the capacity like "hello world".
    let invalid = if cfg!(windows) {
        Outcome::PartialTail
    } else {
        Outcome::Same
    };
    assert_eq!(utf8_to_wcs_both(b"\xFFhello", 0, Some(5), 0), invalid);
}

/// `bnum_allocs` after each `_ptr` call and after freeing its result, on
/// both sides, in a child (other tests allocate in parallel).
#[test]
fn ptr_forms_leave_the_same_allocations() {
    if !common::isolated(concat!(
        module_path!(),
        "::ptr_forms_leave_the_same_allocations"
    )) {
        return;
    }
    // SAFETY: the two allocators' counters.
    let counts = || unsafe { (bnum_allocs(), oracle_bnum_allocs()) };
    let base = counts();
    let delta = |now: (c_long, c_long)| (now.0 - base.0, now.1 - base.1);
    let hello = c"hello";
    let bad = c"bad \xFF byte";
    let wide_r = rw(&nulw(&[0x68, 0x69]));
    let wide_c = cw(&nulw(&[0x68, 0x69]));
    for s in [hello.as_ptr(), bad.as_ptr(), c"".as_ptr(), ptr::null()] {
        let s = s.cast::<c_char>();
        // SAFETY: NUL-terminated or NULL; `*pstr` writable; each result is
        // freed by its own side.
        unsafe {
            let (mut br, mut bc): (*mut c_char, *mut c_char) = (ptr::null_mut(), ptr::null_mut());
            rs::os_utf8_to_mbs_ptr(s, 0, &mut br);
            c::oracle_os_utf8_to_mbs_ptr(s, 0, &mut bc);
            let d = delta(counts());
            assert_eq!(d.0, d.1, "utf8_to_mbs_ptr");
            bfree(br.cast());
            oracle_bfree(bc.cast());
            rs::os_mbs_to_utf8_ptr(s, 0, &mut br);
            c::oracle_os_mbs_to_utf8_ptr(s, 0, &mut bc);
            let d = delta(counts());
            assert_eq!(d.0, d.1, "mbs_to_utf8_ptr");
            bfree(br.cast());
            oracle_bfree(bc.cast());
            let (mut wr, mut wc): (*mut WChar, *mut CWChar) = (ptr::null_mut(), ptr::null_mut());
            rs::os_utf8_to_wcs_ptr(s, 0, &mut wr);
            c::oracle_os_utf8_to_wcs_ptr(s, 0, &mut wc);
            rs::os_wcs_to_utf8_ptr(wide_r.as_ptr(), 0, &mut br);
            c::oracle_os_wcs_to_utf8_ptr(wide_c.as_ptr(), 0, &mut bc);
            let d = delta(counts());
            assert_eq!(d.0, d.1, "utf8_to_wcs_ptr + wcs_to_utf8_ptr");
            bfree(wr.cast());
            oracle_bfree(wc.cast());
            bfree(br.cast());
            oracle_bfree(bc.cast());
        }
        assert_eq!(delta(counts()), (0, 0));
    }
}

/// The C locale proptests, and the UTF-8 locale ones in a child.
#[test]
fn mbs_match_oracle_in_a_utf8_locale() {
    if !common::isolated(concat!(
        module_path!(),
        "::mbs_match_oracle_in_a_utf8_locale"
    )) {
        return;
    }
    let found = [c"C.UTF-8", c"en_US.UTF-8", c".UTF8", c"UTF-8"]
        .iter()
        // SAFETY: a NUL-terminated name; the child runs one test.
        .any(|name: &&CStr| !unsafe { libc::setlocale(libc::LC_CTYPE, name.as_ptr()) }.is_null());
    if !found {
        eprintln!("no UTF-8 locale installed");
        return;
    }
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig::with_cases(256));
    runner
        .run(
            &(utf8_text(), wide_text(), any::<usize>()),
            |(text, units, junk)| {
                mbs_cases(&text, &units, junk);
                ptr_forms_both(&text, &units, text.len(), units.len(), false);
                Ok(())
            },
        )
        .unwrap();
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn utf8_to_wcs_matches_oracle(text in utf8_text(), junk in any::<usize>()) {
        utf8_cases(&text, junk);
    }

    #[test]
    fn wcs_to_utf8_matches_oracle(units in wide_text(), junk in any::<usize>()) {
        wcs_utf8_cases(&units, junk);
    }

    #[test]
    fn mbs_match_oracle_in_the_c_locale(
        text in utf8_text(),
        units in wide_text(),
        junk in any::<usize>(),
    ) {
        mbs_cases(&text, &units, junk);
    }

    #[test]
    fn ptr_forms_match_oracle(
        text in utf8_text(),
        units in wide_text(),
        len in 0usize..8,
        wlen in 0usize..8,
    ) {
        let len = len.min(text.len());
        let wlen = wlen.min(units.len());
        ptr_forms_both(&text, &units, len, wlen, false);
    }
}
