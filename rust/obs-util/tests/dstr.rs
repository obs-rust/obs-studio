//! Tier 1: safe-core tests for `libobs/util/dstr.c`.
//!
//! 1:1 port of the assertions of `test/cmocka/test_dstr.c` that exercise a
//! function ported to Rust, plus edge cases. Skipped C tests (they cover only
//! `dstr-libc.c` functions or header-inline helpers that stay in C):
//! `test_dstr_case` (`dstr_to_upper`/`dstr_to_lower`), `test_dstr_printf`
//! (`dstr_printf`/`dstr_catf`) and `test_dstr_inline_helpers` (`dstr_cmp`,
//! `dstr_find`, `dstr_end`, ...). The leak check of the C tests is replaced
//! by ownership.
//!
//! Ports of `test/cmocka/test_dstr_regressions.c` (the C fixes for #46 and
//! #47 in PR #53) are covered below as well: `dstr_insert_ch` stays within
//! its buffer when `capacity == len + 2`, and `dstr_replace` with an empty
//! `find` is a no-op.

use obs_c_oracle as _;
use obs_util::dstr::{Dstr, astrcmp_n, astrcmpi, astrcmpi_n, astrstri, strdepad, strlist_split};

fn s(x: &str) -> Option<&[u8]> {
    Some(x.as_bytes())
}

fn dstr_of(x: &str) -> Dstr {
    let mut d = Dstr::new();
    d.copy(x.as_bytes());
    d
}

#[track_caller]
fn check(d: &Dstr, expect: &str) {
    assert_eq!(d.as_bytes(), expect.as_bytes());
    assert_eq!(d.len(), expect.len());
    assert!(d.capacity() > d.len());
}

#[track_caller]
fn check_freed(d: &Dstr) {
    assert!(d.as_bytes().is_empty());
    assert_eq!(d.len(), 0);
    assert_eq!(d.capacity(), 0);
}

#[test]
fn test_astrcmpi() {
    assert_eq!(astrcmpi(s("abc"), s("abc")), 0);
    assert_eq!(astrcmpi(s("abc"), s("ABC")), 0);
    assert_eq!(astrcmpi(s("aBc"), s("AbC")), 0);
    assert_eq!(astrcmpi(s("a"), s("b")), -1);
    assert_eq!(astrcmpi(s("B"), s("a")), 1);
    assert_eq!(astrcmpi(s("ab"), s("abc")), -1);
    assert_eq!(astrcmpi(s("abc"), s("ab")), 1);
    assert_eq!(astrcmpi(s(""), s("")), 0);

    // NULL is treated as the empty string
    assert_eq!(astrcmpi(None, None), 0);
    assert_eq!(astrcmpi(None, s("")), 0);
    assert_eq!(astrcmpi(s(""), None), 0);
    assert_eq!(astrcmpi(None, s("a")), -1);
    assert_eq!(astrcmpi(s("a"), None), 1);
}

#[test]
fn test_astrcmp_n() {
    // n == 0 always equal
    assert_eq!(astrcmp_n(s("a"), s("b"), 0), 0);
    assert_eq!(astrcmp_n(None, s("b"), 0), 0);

    assert_eq!(astrcmp_n(s("abcd"), s("abce"), 3), 0);
    assert_eq!(astrcmp_n(s("abcd"), s("abce"), 4), -1);
    assert_eq!(astrcmp_n(s("abce"), s("abcd"), 4), 1);
    assert_eq!(astrcmp_n(s("abc"), s("abc"), 100), 0);
    assert_eq!(astrcmp_n(s("ab"), s("abc"), 100), -1);

    // case sensitive
    assert_eq!(astrcmp_n(s("A"), s("a"), 1), -1);
    assert_eq!(astrcmp_n(s("a"), s("A"), 1), 1);

    // NULL is treated as the empty string
    assert_eq!(astrcmp_n(None, None, 5), 0);
    assert_eq!(astrcmp_n(None, s("a"), 1), -1);
    assert_eq!(astrcmp_n(s("a"), None, 1), 1);
}

#[test]
fn test_astrcmpi_n() {
    assert_eq!(astrcmpi_n(s("a"), s("b"), 0), 0);
    assert_eq!(astrcmpi_n(None, s("b"), 0), 0);

    assert_eq!(astrcmpi_n(s("ABCD"), s("abce"), 3), 0);
    assert_eq!(astrcmpi_n(s("ABCD"), s("abce"), 4), -1);
    assert_eq!(astrcmpi_n(s("abce"), s("ABCD"), 4), 1);
    assert_eq!(astrcmpi_n(s("ABC"), s("abc"), 100), 0);
    assert_eq!(astrcmpi_n(s("AB"), s("abc"), 100), -1);

    assert_eq!(astrcmpi_n(None, None, 5), 0);
    assert_eq!(astrcmpi_n(None, s("a"), 1), -1);
    assert_eq!(astrcmpi_n(s("a"), None, 1), 1);
}

#[test]
fn compare_uses_c_char_ordering() {
    // bytes >= 0x80 order as C `char`: below 'a' where char is signed
    let high: &[u8] = &[0x80];
    let expect = if (0x80u8 as core::ffi::c_char) < (b'a' as core::ffi::c_char) {
        -1
    } else {
        1
    };
    assert_eq!(astrcmp_n(Some(high), s("a"), 1), expect);
    assert_eq!(astrcmpi(Some(high), s("a")), expect);
    assert_eq!(astrcmpi_n(Some(high), s("A"), 1), expect);
}

#[test]
fn compare_stops_at_nul() {
    assert_eq!(astrcmpi(Some(b"ab\0zz"), Some(b"AB\0yy")), 0);
    assert_eq!(astrcmp_n(Some(b"ab\0zz"), Some(b"ab\0yy"), 5), 0);
}

#[test]
fn test_astrstri() {
    let h = "Hello World";

    assert_eq!(astrstri(s(h), s("WORLD")), Some(6));
    assert_eq!(astrstri(s(h), s("llo")), Some(2));
    assert_eq!(astrstri(s(h), s("hello world")), Some(0));
    assert_eq!(astrstri(s(h), s("d")), Some(10));
    assert_eq!(astrstri(s(h), s("xyz")), None);
    assert_eq!(astrstri(s(h), s("Hello World!")), None);
    assert_eq!(astrstri(s(""), s("a")), None);

    // empty needle matches at the start
    assert_eq!(astrstri(s(h), s("")), Some(0));
    assert_eq!(astrstri(s(""), s("")), Some(0));

    assert_eq!(astrstri(None, s("a")), None);
    assert_eq!(astrstri(s(h), None), None);
    assert_eq!(astrstri(None, None), None);
}

#[test]
fn astrstri_edge_cases() {
    // the needle ends at its NUL, the haystack at its NUL
    assert_eq!(astrstri(Some(b"abc\0def"), Some(b"c\0zzz")), Some(2));
    assert_eq!(astrstri(Some(b"abc\0def"), s("def")), None);
    assert_eq!(astrstri(s("aab"), s("ab")), Some(1));
}

fn depad(x: &[u8]) -> Vec<u8> {
    let mut buf = x.to_vec();
    buf.push(0);
    let n = strdepad(&mut buf);
    assert_eq!(buf[n], 0);
    buf.truncate(n);
    buf
}

#[test]
fn test_strdepad() {
    assert_eq!(depad(b"  \t hi there \r\n"), b"hi there");
    assert_eq!(depad(b"nopad"), b"nopad");
    assert_eq!(depad(b"   lead"), b"lead");
    assert_eq!(depad(b"trail \t\n"), b"trail");
    assert_eq!(depad(b" \t\r\n "), b"");
    assert_eq!(depad(b""), b"");
}

#[test]
fn strdepad_edge_cases() {
    // only space, tab, CR and LF are padding
    assert_eq!(depad(b"\x0b a \x0c"), b"\x0b a \x0c");
    // the string ends at the first NUL
    assert_eq!(depad(b" a \0 b "), b"a");
    // a buffer without a terminator ends at its end
    let mut buf = *b"  ab ";
    assert_eq!(strdepad(&mut buf), 2);
    assert_eq!(&buf[..2], b"ab");
}

fn split(x: Option<&str>, ch: u8, include_empty: bool) -> Vec<String> {
    strlist_split(x.unwrap().as_bytes(), ch, include_empty)
        .into_iter()
        .map(|p| String::from_utf8(p.to_vec()).unwrap())
        .collect()
}

#[test]
fn test_strlist_split() {
    assert_eq!(split(s_str("a,b,,c"), b',', true), ["a", "b", "", "c"]);
    assert_eq!(split(s_str("a,b,,c"), b',', false), ["a", "b", "c"]);

    // trailing separator
    assert_eq!(split(s_str("a,b,"), b',', true), ["a", "b", ""]);
    assert_eq!(split(s_str("a,b,"), b',', false), ["a", "b"]);

    // leading separator
    assert_eq!(split(s_str(",a"), b',', true), ["", "a"]);

    // no separator present
    assert_eq!(split(s_str("abc"), b',', false), ["abc"]);

    // empty input
    assert_eq!(split(s_str(""), b',', true), [""]);
    assert!(split(s_str(""), b',', false).is_empty());
}

fn s_str(x: &str) -> Option<&str> {
    Some(x)
}

#[test]
fn strlist_split_edge_cases() {
    // only the C string before the first NUL is split
    assert_eq!(strlist_split(b"a,b\0c,d", b',', true), [b"a", b"b"]);
    // all separators
    assert_eq!(strlist_split(b",,,", b',', true), [b"", b"", b"", b""]);
    assert!(strlist_split(b",,,", b',', false).is_empty());
    // a NUL separator finds nothing (C is undefined here)
    assert_eq!(strlist_split(b"abc", 0, true), [b"abc"]);
}

#[test]
fn test_dstr_copy() {
    let mut d = Dstr::new();

    d.copy(b"hello");
    check(&d, "hello");

    // copy shorter over longer
    d.copy(b"hi");
    check(&d, "hi");

    // copy longer
    d.copy(b"a considerably longer string");
    check(&d, "a considerably longer string");
    assert_eq!(d.len(), 28);

    // NULL and empty free the destination
    d.copy(b"");
    check_freed(&d);

    d.copy(b"x");
    check(&d, "x");
    d.copy(b"");
    check_freed(&d);
}

#[test]
fn dstr_copy_capacity_follows_ensure_capacity() {
    let mut d = Dstr::new();
    d.copy(b"hello");
    assert_eq!(d.capacity(), 6);
    // a shorter copy keeps the capacity
    d.copy(b"hi");
    assert_eq!(d.capacity(), 6);
    // growth doubles when that is enough ...
    d.copy(b"0123456");
    assert_eq!(d.capacity(), 12);
    // ... and otherwise takes the requested size
    d.copy(&[b'x'; 40]);
    assert_eq!(d.capacity(), 41);
    // a NUL ends the copied string
    d.copy(b"ab\0cd");
    assert_eq!(d.as_bytes(), b"ab");
}

#[test]
fn test_dstr_ncopy() {
    let mut d = Dstr::new();

    d.ncopy(&b"hello world"[..5]);
    assert_eq!(d.as_bytes(), b"hello");
    assert_eq!(d.len(), 5);
    assert_eq!(d.capacity(), 6);

    // previous contents are replaced
    d.ncopy(&b"abcdef"[..2]);
    assert_eq!(d.as_bytes(), b"ab");
    assert_eq!(d.len(), 2);
    assert_eq!(d.capacity(), 3);

    // zero length frees
    d.ncopy(b"");
    check_freed(&d);

    let src = dstr_of("abcdef");
    d.ncopy_dstr(src.as_bytes(), 3);
    assert_eq!(d.as_bytes(), b"abc");
    assert_eq!(d.len(), 3);
    assert_eq!(d.capacity(), 4);

    // length is clamped to the source length
    d.ncopy_dstr(src.as_bytes(), 100);
    assert_eq!(d.as_bytes(), b"abcdef");
    assert_eq!(d.len(), 6);
    assert_eq!(d.capacity(), 7);

    d.ncopy_dstr(src.as_bytes(), 0);
    check_freed(&d);
}

#[test]
fn ncopy_keeps_interior_nul() {
    let mut d = Dstr::new();
    d.ncopy(b"a\0b");
    assert_eq!(d.as_bytes(), b"a\0b");
    assert_eq!(d.capacity(), 4);
}

#[test]
fn test_dstr_cat() {
    let mut d = Dstr::new();
    let mut other = Dstr::new();

    // empty is a no-op and does not allocate
    d.cat(b"");
    check_freed(&d);

    d.cat(b"foo");
    check(&d, "foo");

    d.cat(b"bar");
    check(&d, "foobar");

    d.cat_ch(b'!');
    check(&d, "foobar!");

    d.ncat(&b"123456"[..3]);
    check(&d, "foobar!123");
    assert_eq!(d.len(), 10);

    // zero length / empty are no-ops
    d.ncat(b"");
    d.ncat(b"\0");
    d.ncat(b"\0zz");
    assert_eq!(d.as_bytes(), b"foobar!123");
    assert_eq!(d.len(), 10);

    other.copy(b"-tail");
    d.cat_dstr(other.as_bytes());
    check(&d, "foobar!123-tail");
    assert_eq!(d.len(), 15);

    // empty source is a no-op
    other.free();
    d.cat_dstr(other.as_bytes());
    check(&d, "foobar!123-tail");

    other.copy(b"abcdef");
    d.ncat_dstr(other.as_bytes(), 2);
    assert_eq!(d.as_bytes(), b"foobar!123-tailab");
    assert_eq!(d.len(), 17);

    // length is clamped to the source length
    d.ncat_dstr(other.as_bytes(), 100);
    check(&d, "foobar!123-tailababcdef");
    assert_eq!(d.len(), 23);
}

#[test]
fn dstr_cat_capacity_follows_ensure_capacity() {
    let mut d = Dstr::new();
    d.cat(b"foo");
    assert_eq!(d.capacity(), 4);
    d.cat(b"bar");
    assert_eq!(d.capacity(), 8);
    d.cat_ch(b'!');
    assert_eq!(d.capacity(), 8);
    d.ncat(b"123");
    assert_eq!(d.capacity(), 16);
    d.ncat(&[b'z'; 30]);
    assert_eq!(d.capacity(), 41);
}

#[test]
fn ncat_dstr_with_leading_nul_is_noop() {
    let mut d = dstr_of("ab");
    d.ncat_dstr(b"\0cd", 3);
    assert_eq!(d.as_bytes(), b"ab");
    d.ncat_dstr(b"cd", 0);
    assert_eq!(d.as_bytes(), b"ab");
}

#[test]
fn test_dstr_insert() {
    let mut d = Dstr::new();
    let mut other = Dstr::new();

    // inserting at len (0 here) is a cat
    d.insert(0, b"ad");
    assert_eq!(d.as_bytes(), b"ad");
    assert_eq!(d.len(), 2);

    d.insert(1, b"bc");
    check(&d, "abcd");

    d.insert(4, b"ef");
    assert_eq!(d.as_bytes(), b"abcdef");
    assert_eq!(d.len(), 6);

    d.insert(0, b"X");
    check(&d, "Xabcdef");

    // empty is a no-op
    d.insert(2, b"");
    d.insert(2, b"\0zz");
    assert_eq!(d.as_bytes(), b"Xabcdef");
    assert_eq!(d.len(), 7);

    other.copy(b"--");
    d.insert_dstr(3, other.as_bytes());
    check(&d, "Xab--cdef");
    assert_eq!(d.len(), 9);

    d.insert_dstr(9, other.as_bytes());
    assert_eq!(d.as_bytes(), b"Xab--cdef--");
    assert_eq!(d.len(), 11);

    other.free();
    d.insert_dstr(1, other.as_bytes());
    assert_eq!(d.as_bytes(), b"Xab--cdef--");
    assert_eq!(d.len(), 11);
}

#[test]
#[should_panic]
fn insert_past_end_panics() {
    let mut d = dstr_of("ab");
    d.insert(3, b"x");
}

#[test]
fn test_dstr_insert_ch() {
    let mut d = Dstr::new();

    // idx == len is a cat_ch, including on an empty dstr
    d.insert_ch(0, b'a');
    assert_eq!(d.as_bytes(), b"a");
    assert_eq!(d.len(), 1);

    d.insert_ch(1, b'c');
    assert_eq!(d.as_bytes(), b"ac");
    assert_eq!(d.len(), 2);

    // The C test reserves room here (it predates the #46 fix); reserve
    // anyway to match it 1:1.
    d.reserve(16);

    d.insert_ch(1, b'b');
    check(&d, "abc");

    d.insert_ch(0, b'X');
    check(&d, "Xabc");

    d.insert_ch(4, b'Z');
    assert_eq!(d.as_bytes(), b"XabcZ");
    assert_eq!(d.len(), 5);
}

/// `insert_ch_stays_in_bounds` from `test_dstr_regressions.c` (issue #46):
/// capacity 4 already fits "abc", so nothing grows.
#[test]
fn insert_ch_stays_in_bounds() {
    let mut d = Dstr::from_parts(b"ac".to_vec(), 4);
    d.insert_ch(1, b'b');
    assert_eq!(d.as_bytes(), b"abc");
    assert_eq!(d.len(), 3);
    assert_eq!(d.capacity(), 4);
}

#[test]
fn insert_ch_grows_when_capacity_is_tight() {
    // Issue #46 territory: growth follows the ensure_capacity rule.
    let mut d = Dstr::new();
    d.ncopy(b"ac");
    assert_eq!(d.capacity(), 3);
    d.insert_ch(1, b'b');
    assert_eq!(d.as_bytes(), b"abc");
    assert_eq!(d.capacity(), 6);

    // capacity 5 holds "abcd" exactly; inserting needs 6 and doubles to 10
    let mut d = Dstr::new();
    d.ncopy(b"abcd");
    assert_eq!(d.capacity(), 5);
    d.insert_ch(0, b'X');
    assert_eq!(d.as_bytes(), b"Xabcd");
    assert_eq!(d.capacity(), 10);
}

#[test]
fn test_dstr_remove() {
    let mut d = dstr_of("abcdef");

    d.remove(1, 0);
    assert_eq!(d.as_bytes(), b"abcdef");
    assert_eq!(d.len(), 6);

    // middle
    d.remove(1, 2);
    check(&d, "adef");

    // tail
    d.remove(2, 2);
    check(&d, "ad");

    // head
    d.remove(0, 1);
    assert_eq!(d.as_bytes(), b"d");
    assert_eq!(d.len(), 1);

    // removing everything frees
    d.remove(0, 1);
    check_freed(&d);
}

#[test]
fn remove_keeps_capacity_and_count_equal_len_frees() {
    let mut d = dstr_of("abcdef");
    let cap = d.capacity();
    d.remove(2, 3);
    assert_eq!(d.as_bytes(), b"abf");
    assert_eq!(d.capacity(), cap);
    // count == len frees whatever idx is
    d.remove(2, 3);
    check_freed(&d);
}

#[test]
#[should_panic]
fn remove_out_of_range_panics() {
    let mut d = dstr_of("abc");
    d.remove(2, 2);
}

#[test]
fn test_dstr_replace() {
    let mut d = Dstr::new();

    // empty dstr is a no-op
    d.replace(b"a", b"b");
    check_freed(&d);

    // replacement shorter than find
    d.copy(b"foo bar foo");
    d.replace(b"foo", b"x");
    check(&d, "x bar x");
    assert_eq!(d.len(), 7);

    // empty replacement
    d.copy(b"foo bar foo");
    d.replace(b"foo", b"");
    assert_eq!(d.as_bytes(), b" bar ");
    assert_eq!(d.len(), 5);

    // replacement longer than find
    d.copy(b"a-b-c");
    d.replace(b"-", b"--");
    check(&d, "a--b--c");
    assert_eq!(d.len(), 7);

    d.copy(b"-start and end-");
    d.replace(b"-", b"<longer>");
    check(&d, "<longer>start and end<longer>");
    assert_eq!(d.len(), 29);

    // replacement same length as find
    d.copy(b"abcabc");
    d.replace(b"abc", b"xyz");
    assert_eq!(d.as_bytes(), b"xyzxyz");
    assert_eq!(d.len(), 6);

    // no match, every branch
    d.copy(b"hello");
    d.replace(b"zzz", b"y");
    assert_eq!(d.as_bytes(), b"hello");
    d.replace(b"zzz", b"yyyyyy");
    assert_eq!(d.as_bytes(), b"hello");
    d.replace(b"zzz", b"yyy");
    assert_eq!(d.as_bytes(), b"hello");
    assert_eq!(d.len(), 5);
}

/// `replace_empty_find_is_noop` from `test_dstr_regressions.c` (issue #47).
#[test]
fn replace_with_empty_find_is_noop() {
    let mut d = dstr_of("hello");
    let before = d.clone();
    d.replace(b"", b"x");
    assert_eq!(d, before);
    d.replace(b"\0abc", b"x");
    assert_eq!(d, before);
}

#[test]
fn replace_capacity_and_overlap() {
    let mut d = dstr_of("a-b-c");
    assert_eq!(d.capacity(), 6);
    // growth: ensure_capacity(new_len + 1) = 8, doubling gives 12
    d.replace(b"-", b"--");
    assert_eq!(d.as_bytes(), b"a--b--c");
    assert_eq!(d.capacity(), 12);
    // shrinking never changes the capacity
    d.replace(b"--", b"+");
    assert_eq!(d.as_bytes(), b"a+b+c");
    assert_eq!(d.capacity(), 12);
    // matches do not overlap and replaced text is not rescanned
    d.copy(b"aaaa");
    d.replace(b"aa", b"a");
    assert_eq!(d.as_bytes(), b"aa");
    d.copy(b"abab");
    d.replace(b"ab", b"abab");
    assert_eq!(d.as_bytes(), b"abababab");
    // a replacement that empties the string keeps the allocation
    d.copy(b"abc");
    let cap = d.capacity();
    d.replace(b"abc", b"");
    assert_eq!(d.len(), 0);
    assert_eq!(d.capacity(), cap);
    assert!(d.is_empty());
}

#[test]
fn test_dstr_depad() {
    let mut d = Dstr::new();

    // no array is a no-op
    d.depad();
    check_freed(&d);

    d.copy(b"  \thi there \r\n");
    d.depad();
    check(&d, "hi there");
    assert_eq!(d.len(), 8);

    d.copy(b"tight");
    d.depad();
    assert_eq!(d.as_bytes(), b"tight");
    assert_eq!(d.len(), 5);

    // all whitespace frees
    d.copy(b" \t\r\n ");
    d.depad();
    check_freed(&d);
}

#[test]
fn depad_keeps_capacity_and_reads_c_string() {
    let mut d = dstr_of("  ab  ");
    let cap = d.capacity();
    d.depad();
    assert_eq!(d.as_bytes(), b"ab");
    assert_eq!(d.capacity(), cap);

    // len follows strlen of the depadded array
    let mut d = Dstr::new();
    d.ncopy(b" a\0b ");
    d.depad();
    assert_eq!(d.as_bytes(), b"a");
}

#[test]
fn test_dstr_left_mid_right() {
    let src = dstr_of("hello world");
    let mut d = Dstr::new();

    d.left(src.as_bytes(), 5);
    check(&d, "hello");

    // in-place (dst == src)
    let mut same = dstr_of("hello world");
    same.left_in_place(8);
    check(&same, "hello wo");

    d.mid(src.as_bytes(), 6, 5);
    check(&d, "world");

    d.mid(src.as_bytes(), 2, 3);
    assert_eq!(d.as_bytes(), b"llo");
    assert_eq!(d.len(), 3);

    // zero count frees dst
    d.mid(src.as_bytes(), 2, 0);
    check_freed(&d);

    d.right(src.as_bytes(), 6);
    check(&d, "world");

    d.right(src.as_bytes(), 0);
    assert_eq!(d.as_bytes(), b"hello world");
    assert_eq!(d.len(), 11);

    // pos == len leaves dst empty
    d.right(src.as_bytes(), 11);
    check_freed(&d);
}

#[test]
fn left_mid_right_capacities() {
    let src = dstr_of("hello world");
    let mut d = Dstr::new();

    // left: dst grows by ensure_capacity and keeps its capacity otherwise
    d.left(src.as_bytes(), 5);
    assert_eq!(d.capacity(), 6);
    d.left(src.as_bytes(), 2);
    assert_eq!(d.as_bytes(), b"he");
    assert_eq!(d.capacity(), 6);
    d.left(src.as_bytes(), 9);
    assert_eq!(d.capacity(), 12);
    d.left(src.as_bytes(), 0);
    check_freed(&d);

    // mid and right allocate exactly count + 1 / len - pos + 1
    d.mid(src.as_bytes(), 6, 5);
    assert_eq!(d.capacity(), 6);
    d.right(src.as_bytes(), 3);
    assert_eq!(d.capacity(), 9);
}

#[test]
#[should_panic]
fn left_past_end_panics() {
    let src = dstr_of("ab");
    Dstr::new().left(src.as_bytes(), 3);
}

#[test]
fn test_dstr_safe_printf() {
    let mut d = Dstr::new();

    d.safe_printf(
        s("$1 and $2 and $3 and $4"),
        [s("a"), s("bb"), s("ccc"), s("dddd")],
    );
    check(&d, "a and bb and ccc and dddd");
    assert_eq!(d.len(), 25);

    // NULL values leave their placeholder untouched
    d.safe_printf(s("x=$1 y=$2"), [s("1"), None, None, None]);
    assert_eq!(d.as_bytes(), b"x=1 y=$2");
    assert_eq!(d.len(), 8);

    // repeated placeholder
    d.safe_printf(s("$1$1"), [s("ab"), None, None, None]);
    assert_eq!(d.as_bytes(), b"abab");
    assert_eq!(d.len(), 4);

    // empty value removes the placeholder
    d.safe_printf(s("[$1]"), [s(""), None, None, None]);
    assert_eq!(d.as_bytes(), b"[]");
    assert_eq!(d.len(), 2);

    // NULL format frees
    d.safe_printf(None, [s("a"), s("b"), s("c"), s("d")]);
    check_freed(&d);
}

#[test]
fn safe_printf_substitutions_are_not_rescanned() {
    let mut d = Dstr::new();
    // $1 -> "$2" is substituted before $2, so the later pass sees it
    d.safe_printf(s("$1"), [s("$2"), s("X"), None, None]);
    assert_eq!(d.as_bytes(), b"X");
}

#[test]
fn test_dstr_strref() {
    // dstr_copy_strref / dstr_init_copy_strref are ncopy on the referenced
    // bytes; dstr_cat_strref is ncat.
    let mut d = Dstr::new();

    d.ncopy(&b"hello world"[..5]);
    assert_eq!(d.as_bytes(), b"hello");
    assert_eq!(d.len(), 5);
    assert_eq!(d.capacity(), 6);

    // replaces previous contents
    d.ncopy(&b"abcdef"[..3]);
    assert_eq!(d.as_bytes(), b"abc");
    assert_eq!(d.len(), 3);

    d.ncat(&b"abcdef"[..3]);
    check(&d, "abcabc");
    assert_eq!(d.len(), 6);

    // empty strref
    d.ncat(b"");
    assert_eq!(d.as_bytes(), b"abcabc");
    assert_eq!(d.len(), 6);

    d.ncopy(b"");
    check_freed(&d);

    let mut d = Dstr::new();
    d.ncopy(&b"init-copy"[..4]);
    assert_eq!(d.as_bytes(), b"init");
    assert_eq!(d.len(), 4);
    assert_eq!(d.capacity(), 5);
}

#[test]
fn arbitrary_bytes_are_preserved() {
    let mut d = Dstr::new();
    d.ncopy(&[0xff, 0xfe, 0x80, 0x00, 0xc3]);
    d.ncat(&[0xc3, 0x28]);
    assert_eq!(d.as_bytes(), &[0xff, 0xfe, 0x80, 0x00, 0xc3, 0xc3, 0x28]);
}

#[test]
fn inline_helper_semantics() {
    let d = Dstr::new();
    assert!(d.is_empty());
    let d = dstr_of("Hello");
    assert!(!d.is_empty());
    // a leading NUL makes the dstr empty
    let mut d = Dstr::new();
    d.ncopy(b"\0a");
    assert!(d.is_empty());
}
