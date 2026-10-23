//! Tier 1: safe-core tests for `libobs/util/dstr-libc.c`.
//!
//! 1:1 port of the assertions of `test/cmocka/test_dstr.c` that exercise the
//! safe core in `src/dstr_libc.rs`: `test_wstrcmpi`, `test_wstrcmp_n`,
//! `test_wstrcmpi_n`, `test_wstrstri`, `test_wcsdepad`, and the case loop of
//! `test_dstr_case`, `test_dstr_case_edges` and `test_dstr_locale`. NULL is
//! the empty string here (the shims map it), and the C library's
//! `towupper` is a closure: [`ascii_up`] stands for the "C" locale, and
//! [`latin_up`] for a UTF-8 locale that maps U+00E9 to U+00C9. Cases that
//! only exercise the conversions or the printf family (which call other C
//! functions, or stay C) are Tier 2 and Tier 3 only. The leak check of the C
//! tests is replaced by ownership.

use obs_c_oracle as _;
use obs_util::dstr_libc::{compare_by, map_case, wcsdepad, wstrstri};

/// `wchar_t` on Linux and macOS.
fn w(s: &str) -> Vec<i32> {
    s.chars().map(|c| c as i32).collect()
}

/// `wchar_t` on Windows (UTF-16 units).
fn w16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// `towupper` in the "C" locale.
fn ascii_up(c: i32) -> i32 {
    if (i32::from(b'a')..=i32::from(b'z')).contains(&c) {
        c - 32
    } else {
        c
    }
}

/// `towupper` in a UTF-8 locale, for the code points used here.
fn latin_up(c: i32) -> i32 {
    match c {
        0xe0..=0xfe if c != 0xf7 => c - 0x20,
        _ => ascii_up(c),
    }
}

fn unit<T: Copy + From<u8>>(s: &[T], i: usize) -> T {
    s.get(i).copied().unwrap_or_else(|| T::from(0))
}

fn cmp(a: Option<&str>, b: Option<&str>, n: Option<usize>, fold: fn(i32) -> i32) -> i32 {
    let (a, b) = (w(a.unwrap_or("")), w(b.unwrap_or("")));
    compare_by(n, fold, |i| (unit(&a, i), unit(&b, i)))
}

fn wstrcmpi(a: Option<&str>, b: Option<&str>) -> i32 {
    cmp(a, b, None, ascii_up)
}

fn wstrcmp_n(a: Option<&str>, b: Option<&str>, n: usize) -> i32 {
    cmp(a, b, Some(n), |c| c)
}

fn wstrcmpi_n(a: Option<&str>, b: Option<&str>, n: usize) -> i32 {
    cmp(a, b, Some(n), ascii_up)
}

fn find(s: &str, f: &str) -> Option<usize> {
    wstrstri(&w(s), &w(f), ascii_up)
}

/// Runs `wcsdepad` on `s` written over a buffer of `fill` units, and
/// returns the whole buffer.
fn depad_buf(s: &str, fill: &[i32]) -> Vec<i32> {
    let mut buf = fill.to_vec();
    let src = w(s);
    buf[..src.len()].copy_from_slice(&src);
    buf[src.len()] = 0;
    wcsdepad(&mut buf[..=src.len()]);
    buf
}

/// The string `wcsdepad` leaves in `s`.
fn depad(s: &str) -> String {
    let mut buf = w(s);
    buf.push(0);
    wcsdepad(&mut buf);
    buf.iter()
        .take_while(|&&c| c != 0)
        .map(|&c| char::from_u32(c as u32).unwrap())
        .collect()
}

#[test]
fn test_wstrcmpi() {
    let s = Some;
    assert_eq!(wstrcmpi(s("abc"), s("abc")), 0);
    assert_eq!(wstrcmpi(s("abc"), s("ABC")), 0);
    assert_eq!(wstrcmpi(s("aBc"), s("AbC")), 0);
    assert_eq!(wstrcmpi(s("a"), s("b")), -1);
    assert_eq!(wstrcmpi(s("B"), s("a")), 1);
    assert_eq!(wstrcmpi(s("ab"), s("abc")), -1);
    assert_eq!(wstrcmpi(s("abc"), s("ab")), 1);
    assert_eq!(wstrcmpi(s(""), s("")), 0);

    // both sides are compared upper-cased: '[' (0x5b) sorts after 'A'
    assert_eq!(wstrcmpi(s("["), s("a")), 1);
    assert_eq!(wstrcmpi(s("a"), s("[")), -1);

    // NULL is treated as the empty string
    assert_eq!(wstrcmpi(None, None), 0);
    assert_eq!(wstrcmpi(None, s("")), 0);
    assert_eq!(wstrcmpi(s(""), None), 0);
    assert_eq!(wstrcmpi(None, s("a")), -1);
    assert_eq!(wstrcmpi(s("a"), None), 1);
}

#[test]
fn test_wstrcmp_n() {
    let s = Some;
    assert_eq!(wstrcmp_n(s("abc"), s("abd"), 2), 0);
    assert_eq!(wstrcmp_n(s("abc"), s("abd"), 3), -1);
    assert_eq!(wstrcmp_n(s("abd"), s("abc"), 3), 1);
    assert_eq!(wstrcmp_n(s("abc"), s("abc"), 10), 0);
    assert_eq!(wstrcmp_n(s("ab"), s("abc"), 5), -1);
    assert_eq!(wstrcmp_n(s("abc"), s("ab"), 5), 1);

    // case-sensitive: 'a' (0x61) > 'A' (0x41)
    assert_eq!(wstrcmp_n(s("a"), s("A"), 1), 1);
    assert_eq!(wstrcmp_n(s("A"), s("a"), 1), -1);

    // n == 0 compares nothing, even NULL against a string
    assert_eq!(wstrcmp_n(s("a"), s("b"), 0), 0);
    assert_eq!(wstrcmp_n(None, s("b"), 0), 0);

    // NULL is treated as the empty string
    assert_eq!(wstrcmp_n(None, None, 4), 0);
    assert_eq!(wstrcmp_n(None, s(""), 4), 0);
    assert_eq!(wstrcmp_n(None, s("x"), 1), -1);
    assert_eq!(wstrcmp_n(s("x"), None, 1), 1);
}

#[test]
fn test_wstrcmpi_n() {
    let s = Some;
    assert_eq!(wstrcmpi_n(s("abc"), s("ABD"), 2), 0);
    assert_eq!(wstrcmpi_n(s("abc"), s("ABD"), 3), -1);
    assert_eq!(wstrcmpi_n(s("ABD"), s("abc"), 3), 1);
    assert_eq!(wstrcmpi_n(s("Hello"), s("hELLO"), 100), 0);
    assert_eq!(wstrcmpi_n(s("ab"), s("ABC"), 5), -1);
    assert_eq!(wstrcmpi_n(s("["), s("a"), 1), 1);

    assert_eq!(wstrcmpi_n(s("a"), s("b"), 0), 0);
    assert_eq!(wstrcmpi_n(None, None, 3), 0);
    assert_eq!(wstrcmpi_n(None, s("a"), 1), -1);
    assert_eq!(wstrcmpi_n(s("a"), None, 1), 1);
}

#[test]
fn test_wstrstri() {
    let s = "Hello World";
    assert_eq!(find(s, "world"), Some(6));
    assert_eq!(find(s, "HELLO"), Some(0));
    assert_eq!(find(s, "d"), Some(10));
    assert_eq!(find(s, "lo w"), Some(3));
    assert_eq!(find(s, "xyz"), None);
    assert_eq!(find(s, "Hello World!"), None);

    // the first match wins
    assert_eq!(find("xABab", "ab"), Some(1));

    // an empty find matches at the start, also of an empty string
    assert_eq!(find(s, ""), Some(0));
    assert_eq!(find("", ""), Some(0));
    assert_eq!(find("", "a"), None);
    // (NULL on either side is handled by the shim: NULL)
}

#[test]
fn test_wcsdepad() {
    assert_eq!(depad("  \t hi there \r\n"), "hi there");
    assert_eq!(depad("nopad"), "nopad");
    assert_eq!(depad("   lead"), "lead");
    assert_eq!(depad("trail \t\n"), "trail");

    // only ' ', '\t', '\n' and '\r' are padding; '\v' and '\f' stay
    assert_eq!(depad("\u{b} x \u{c}"), "\u{b} x \u{c}");

    // all padding leaves an empty string
    assert_eq!(depad(" \t\r\n "), "");
    assert_eq!(depad(""), "");

    // the leading padding is moved out with memmove (string and NUL); the
    // trailing padding is overwritten with NULs one by one, so the old tail
    // past them is left as it was
    let hash = i32::from(b'#');
    let buf = depad_buf("  ab  ", &[hash; 8]);
    let sp = i32::from(b' ');
    let (a, b) = (i32::from(b'a'), i32::from(b'b'));
    assert_eq!(buf, [a, b, 0, 0, 0, sp, 0, hash]);
    // (NULL is handled by the shim: NULL)
}

/// `dstr_to_upper` / `dstr_to_lower`: the per-unit loop.
#[test]
fn test_dstr_case() {
    let mut x = w("Hello, World 123");
    map_case(&mut x, ascii_up);
    assert_eq!(x, w("HELLO, WORLD 123"));

    // empty is a no-op
    let mut e: Vec<i32> = Vec::new();
    map_case(&mut e, ascii_up);
    assert!(e.is_empty());
}

#[test]
fn test_dstr_case_edges() {
    // the case is mapped up to the first NUL
    let mut x = vec![i32::from(b'a'), 0, i32::from(b'b'), 0];
    map_case(&mut x, ascii_up);
    assert_eq!(x, [i32::from(b'A'), 0, i32::from(b'b'), 0]);

    // a long string
    let mut long = vec![i32::from(b'm'); 10000];
    map_case(&mut long, ascii_up);
    assert!(long.iter().all(|&c| c == i32::from(b'M')));
}

/// The locale cases of `test_dstr_locale` and `test_dstr_c_locale`: the
/// result is whatever the passed `towupper` maps, one unit at a time.
#[test]
fn test_dstr_locale() {
    let mut x = w("h\u{e9}llo");
    map_case(&mut x, latin_up);
    assert_eq!(x, w("H\u{c9}LLO"));

    // one-to-one: U+00DF has no single upper case, so it stays (no "SS")
    let mut x = w("stra\u{df}e");
    map_case(&mut x, latin_up);
    assert_eq!(x, w("STRA\u{df}E"));

    assert_eq!(
        cmp(Some("\u{e9}t\u{e9}"), Some("\u{c9}T\u{c9}"), None, latin_up),
        0
    );
    assert_eq!(cmp(Some("\u{e9}x"), Some("\u{c9}y"), Some(1), latin_up), 0);
    assert_eq!(wstrstri(&w("caf\u{e9}"), &w("\u{c9}"), latin_up), Some(3));

    // the "C" locale leaves U+00E9 alone
    let mut x = w("h\u{e9}llo");
    map_case(&mut x, ascii_up);
    assert_eq!(x, w("H\u{e9}LLO"));
    assert_eq!(wstrcmpi(Some("\u{e9}"), Some("\u{c9}")), 1);
}

// ---------------------------------------------------------------------------
// New edge cases

/// Windows `wchar_t` is a 16-bit unsigned unit; the same code runs on it.
#[test]
fn utf16_units() {
    let up = |c: u16| if (97..=122).contains(&c) { c - 32 } else { c };
    let (a, b) = (w16("Hello \u{1f600}"), w16("hELLO \u{1f600}"));
    assert_eq!(compare_by(None, up, |i| (unit(&a, i), unit(&b, i))), 0);
    // surrogates (0xd800..) sort above ASCII as unsigned units
    let (a, b) = (w16("\u{1f600}"), w16("a"));
    assert_eq!(compare_by(None, up, |i| (unit(&a, i), unit(&b, i))), 1);
    assert_eq!(wstrstri(&w16("ab\u{1f600}CD"), &w16("cd"), up), Some(4));
    let mut buf = w16(" \t\u{1f600} \n");
    buf.push(0);
    wcsdepad(&mut buf);
    assert_eq!(&buf[..3], &[0xd83d, 0xde00, 0]);
}

/// Linux `wchar_t` is signed: a negative unit sorts below NUL, so a
/// longer string can compare less than its prefix.
#[test]
fn signed_units() {
    let (a, b) = (vec![i32::from(b'a'), -5], vec![i32::from(b'a')]);
    assert_eq!(
        compare_by(None, ascii_up, |i| (unit(&a, i), unit(&b, i))),
        -1
    );
    assert_eq!(
        compare_by(Some(1), ascii_up, |i| (unit(&a, i), unit(&b, i))),
        0
    );
}

/// The compare reads in order and stops at the first difference, the first
/// NUL, or `n`, so a bounded compare never reads past what C reads.
#[test]
fn compare_reads_no_further_than_c() {
    let mut reads = Vec::new();
    let a = w("abcdef");
    let b = w("abXdef");
    let r = compare_by(
        Some(5),
        |c| c,
        |i| {
            reads.push(i);
            (unit(&a, i), unit(&b, i))
        },
    );
    assert_eq!(r, 1);
    assert_eq!(reads, [0, 1, 2]);

    reads.clear();
    compare_by(
        Some(2),
        |c| c,
        |i| {
            reads.push(i);
            (unit(&a, i), unit(&a, i))
        },
    );
    assert_eq!(reads, [0, 1]);

    reads.clear();
    let short = w("ab");
    compare_by(
        None,
        |c| c,
        |i| {
            reads.push(i);
            (unit(&short, i), unit(&short, i))
        },
    );
    assert_eq!(reads, [0, 1, 2]);
}

/// A `fold` that maps a unit to NUL makes C match at the terminator; the
/// core tries that position too.
#[test]
fn wstrstri_tries_the_terminator() {
    let to_nul = |c: i32| if c == i32::from(b'z') { 0 } else { c };
    assert_eq!(wstrstri(&w("ab"), &w("z"), to_nul), Some(2));
}

#[test]
fn wcsdepad_padding_only_at_the_ends() {
    assert_eq!(depad(" a \t b "), "a \t b");
    assert_eq!(depad("\r"), "");
    assert_eq!(depad("x"), "x");
    assert_eq!(depad(" x"), "x");
    assert_eq!(depad("x "), "x");
}
