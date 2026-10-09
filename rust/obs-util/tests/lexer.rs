//! Tier 1: safe-API port of `test/cmocka/test_lexer.c` (one test per C test
//! function, assertion for assertion) plus edge cases. The C test file is the
//! characterization record; quirks it pins are marked "characterized, not
//! endorsed". The #48 empty-strref asymmetry was fixed in C (PR #60) and
//! the port follows the fixed C. Header-inline helpers (`lexer_init`, `strref_set`,
//! `error_data_free`, ...) stay C and are not part of the safe API; the NULL
//! out-pointer / NULL `error_data` / never-started-lexer cases of the C test
//! only exist at the ABI and are covered in `lexer_parity.rs`.

use core::ffi::c_char;

use obs_c_oracle as _;
use obs_util::lexer::{
    BaseTokenType, ErrorItem, IgnoreWhitespace, Lexer, build_error_string, strref_cmp,
    strref_cmp_strref, strref_cmpi, strref_cmpi_strref, valid_float_str, valid_int_str,
};

/// A non-empty `strref` over `s`.
fn sr(s: &[u8]) -> Option<&[u8]> {
    Some(s)
}

/// A C string argument.
fn cs(s: &[u8]) -> Option<&[u8]> {
    Some(s)
}

#[test]
fn strref_cmp_test() {
    let abc = sr(b"abc");
    let ab = sr(b"ab");
    let empty = None;
    // a strref is a segment: only the first two chars of the array count
    let prefix = sr(&b"abcdef"[..2]);

    assert_eq!(strref_cmp(abc, cs(b"abc")), 0);
    assert_eq!(strref_cmp(abc, cs(b"abd")), -1);
    assert_eq!(strref_cmp(abc, cs(b"abb")), 1);
    // strref shorter than the C string, and longer than it
    assert_eq!(strref_cmp(ab, cs(b"abc")), -1);
    assert_eq!(strref_cmp(abc, cs(b"ab")), 1);
    assert_eq!(strref_cmp(prefix, cs(b"ab")), 0);
    assert_eq!(strref_cmp(prefix, cs(b"abc")), -1);

    // case sensitive: 'A' (65) < 'a' (97)
    let upper = sr(b"ABC");
    assert_eq!(strref_cmp(upper, cs(b"abc")), -1);
    assert_eq!(strref_cmp(abc, cs(b"ABC")), 1);

    // empty strref
    assert_eq!(strref_cmp(empty, cs(b"")), 0);
    assert_eq!(strref_cmp(empty, None), 0);
    assert_eq!(strref_cmp(empty, cs(b"a")), -1);

    // non-empty strref against NULL behaves like against ""
    assert_eq!(strref_cmp(abc, None), 1);
    // NULL strref pointer counts as empty
    assert_eq!(strref_cmp(None, cs(b"")), 0);
    assert_eq!(strref_cmp(None, cs(b"a")), -1);
}

#[test]
fn strref_cmpi_test() {
    let abc = sr(b"abc");
    let upper = sr(b"ABC");
    let empty = None;

    assert_eq!(strref_cmpi(abc, cs(b"ABC")), 0);
    assert_eq!(strref_cmpi(upper, cs(b"abc")), 0);
    assert_eq!(strref_cmpi(upper, cs(b"abd")), -1);
    assert_eq!(strref_cmpi(upper, cs(b"abb")), 1);
    assert_eq!(strref_cmpi(abc, cs(b"ab")), 1);
    assert_eq!(strref_cmpi(abc, cs(b"abcd")), -1);
    assert_eq!(strref_cmpi(abc, None), 1);

    assert_eq!(strref_cmpi(empty, cs(b"")), 0);
    assert_eq!(strref_cmpi(empty, None), 0);
    assert_eq!(strref_cmpi(empty, cs(b"a")), -1);
}

#[test]
fn strref_cmp_strref_test() {
    let abc = sr(b"abc");
    let abc2 = sr(&b"abcxyz"[..3]);
    let abd = sr(b"abd");
    let ab = sr(b"ab");
    let upper = sr(b"ABC");
    let empty = None;
    let empty2 = None;

    assert_eq!(strref_cmp_strref(abc, abc), 0);
    assert_eq!(strref_cmp_strref(abc, abc2), 0);
    assert_eq!(strref_cmp_strref(abc, abd), -1);
    assert_eq!(strref_cmp_strref(abd, abc), 1);
    assert_eq!(strref_cmp_strref(ab, abc), -1);
    assert_eq!(strref_cmp_strref(abc, ab), 1);
    assert_eq!(strref_cmp_strref(upper, abc), -1);
    assert_eq!(strref_cmp_strref(abc, upper), 1);

    // empty handling: an empty string sorts before any non-empty one, in
    // both argument orders (it used to return -1 both ways, #48)
    assert_eq!(strref_cmp_strref(empty, empty2), 0);
    assert_eq!(strref_cmp_strref(empty, abc), -1);
    assert_eq!(strref_cmp_strref(abc, empty), 1);
}

#[test]
fn strref_cmpi_strref_test() {
    let abc = sr(b"abc");
    let upper = sr(b"ABC");
    let abd = sr(b"ABD");
    let ab = sr(b"AB");
    let empty = None;
    let empty2 = None;

    assert_eq!(strref_cmpi_strref(abc, upper), 0);
    assert_eq!(strref_cmpi_strref(upper, abc), 0);
    assert_eq!(strref_cmpi_strref(abc, abd), -1);
    assert_eq!(strref_cmpi_strref(abd, abc), 1);
    assert_eq!(strref_cmpi_strref(ab, abc), -1);
    assert_eq!(strref_cmpi_strref(abc, ab), 1);

    // same empty ordering as strref_cmp_strref (#48)
    assert_eq!(strref_cmpi_strref(empty, empty2), 0);
    assert_eq!(strref_cmpi_strref(empty, abc), -1);
    assert_eq!(strref_cmpi_strref(abc, empty), 1);
}

#[test]
fn valid_int_str_test() {
    assert!(valid_int_str(cs(b"123"), 0));
    assert!(valid_int_str(cs(b"0"), 0));
    assert!(valid_int_str(cs(b"-5"), 0));
    assert!(valid_int_str(cs(b"+7"), 0));
    assert!(!valid_int_str(cs(b"1.5"), 0));
    assert!(!valid_int_str(cs(b"1e3"), 0));
    assert!(!valid_int_str(cs(b"abc"), 0));
    assert!(!valid_int_str(cs(b"12a"), 0));
    assert!(!valid_int_str(cs(b""), 0));
    assert!(!valid_int_str(None, 0));
    // sign only
    assert!(!valid_int_str(cs(b"-"), 0));

    // explicit n limits how many chars are examined
    assert!(valid_int_str(cs(b"12a"), 2));
    assert!(valid_int_str(cs(b"12a"), 1));
    assert!(!valid_int_str(cs(b"12a"), 3));
    // n counts the sign too: "-12x" with n=3 reaches 'x'
    assert!(!valid_int_str(cs(b"-12x"), 3));
    assert!(valid_int_str(cs(b"-12x"), 2));
}

#[test]
fn valid_float_str_test() {
    assert!(valid_float_str(cs(b"123"), 0));
    assert!(valid_float_str(cs(b"1.5"), 0));
    assert!(valid_float_str(cs(b"1e3"), 0));
    assert!(valid_float_str(cs(b"-5"), 0));
    assert!(valid_float_str(cs(b"+1.5e3"), 0));
    assert!(valid_float_str(cs(b"1."), 0));
    assert!(!valid_float_str(cs(b"abc"), 0));
    assert!(!valid_float_str(cs(b""), 0));
    assert!(!valid_float_str(None, 0));
    assert!(!valid_float_str(cs(b".5"), 0));
    assert!(!valid_float_str(cs(b"1.2.3"), 0));
    assert!(!valid_float_str(cs(b"e3"), 0));
    assert!(!valid_float_str(cs(b"1e3e4"), 0));
    assert!(!valid_float_str(cs(b"1e"), 0));
    // quirk (characterized, not endorsed): a sign right after the exponent
    // marker is rejected
    assert!(!valid_float_str(cs(b"1e-3"), 0));
    assert!(!valid_float_str(cs(b"1e+3"), 0));
    // quirk (characterized, not endorsed): a trailing sign after exponent
    // digits is accepted
    assert!(valid_float_str(cs(b"1e3-"), 0));

    // explicit n
    assert!(valid_float_str(cs(b"1.5"), 1));
    assert!(valid_float_str(cs(b"1.x"), 2));
    assert!(!valid_float_str(cs(b"1.x"), 3));
}

/// `expect_token` from the C test: next token has `ty` and spells `text`.
fn expect_token(
    lex: &mut Lexer<'_>,
    src: &[u8],
    iws: IgnoreWhitespace,
    ty: BaseTokenType,
    text: &[u8],
) {
    let t = lex.get_base_token(iws).expect("a token");
    assert_eq!(t.kind, ty);
    assert_eq!(t.len, text.len());
    assert_eq!(strref_cmp(sr(&src[t.start..t.start + t.len]), cs(text)), 0);
}

fn expect_end(lex: &mut Lexer<'_>, iws: IgnoreWhitespace) {
    assert!(lex.get_base_token(iws).is_none());
}

#[test]
fn getbasetoken_ignore_ws_test() {
    use BaseTokenType::*;
    const IGN: IgnoreWhitespace = IgnoreWhitespace::Ignore;
    let src = b"abc 12 +\n x";
    let mut lex = Lexer::new(src);

    expect_token(&mut lex, src, IGN, Alpha, b"abc");
    expect_token(&mut lex, src, IGN, Digit, b"12");
    expect_token(&mut lex, src, IGN, Other, b"+");
    expect_token(&mut lex, src, IGN, Alpha, b"x");
    expect_end(&mut lex, IGN);
    // stays at the end
    expect_end(&mut lex, IGN);

    // lexer_reset rewinds to the start
    lex.reset();
    expect_token(&mut lex, src, IGN, Alpha, b"abc");
}

#[test]
fn getbasetoken_parse_ws_test() {
    use BaseTokenType::*;
    const PARSE: IgnoreWhitespace = IgnoreWhitespace::Parse;
    let src = b"abc 12 +\n x";
    let mut lex = Lexer::new(src);

    expect_token(&mut lex, src, PARSE, Alpha, b"abc");
    expect_token(&mut lex, src, PARSE, Whitespace, b" ");
    expect_token(&mut lex, src, PARSE, Digit, b"12");
    expect_token(&mut lex, src, PARSE, Whitespace, b" ");
    expect_token(&mut lex, src, PARSE, Other, b"+");
    expect_token(&mut lex, src, PARSE, Whitespace, b"\n");
    expect_token(&mut lex, src, PARSE, Whitespace, b" ");
    expect_token(&mut lex, src, PARSE, Alpha, b"x");
    expect_end(&mut lex, PARSE);

    // CRLF and LFCR are single two-char whitespace tokens; lone CR is one char
    let src = b"a\r\nb\n\rc\rd";
    let mut lex = Lexer::new(src);
    expect_token(&mut lex, src, PARSE, Alpha, b"a");
    expect_token(&mut lex, src, PARSE, Whitespace, b"\r\n");
    expect_token(&mut lex, src, PARSE, Alpha, b"b");
    expect_token(&mut lex, src, PARSE, Whitespace, b"\n\r");
    expect_token(&mut lex, src, PARSE, Alpha, b"c");
    expect_token(&mut lex, src, PARSE, Whitespace, b"\r");
    expect_token(&mut lex, src, PARSE, Alpha, b"d");
    expect_end(&mut lex, PARSE);

    // whitespace-only text yields nothing when whitespace is ignored
    let mut lex = Lexer::new(b" \t\n ");
    expect_end(&mut lex, IgnoreWhitespace::Ignore);
    // ...and the cursor ends at the end of the text
    assert_eq!(lex.offset(), 4);

    // an empty text (the closest safe analogue of a never-started lexer)
    let mut lex = Lexer::new(b"");
    expect_end(&mut lex, PARSE);
}

#[test]
fn getstroffset_test() {
    let lex = Lexer::new(b"ab\ncd\r\nef\n\rg");

    assert_eq!(lex.str_offset(0), (1, 1));
    assert_eq!(lex.str_offset(1), (1, 2));
    // 'c' directly after "\n"
    assert_eq!(lex.str_offset(3), (2, 1));
    // 'd'
    assert_eq!(lex.str_offset(4), (2, 2));
    // the '\r' of the CRLF pair is still on row 2, after "cd"
    assert_eq!(lex.str_offset(5), (2, 3));
    // 'e' after "\r\n" counts as a single newline
    assert_eq!(lex.str_offset(7), (3, 1));
    // 'f'
    assert_eq!(lex.str_offset(8), (3, 2));
    // 'g' after "\n\r" counts as a single newline
    assert_eq!(lex.str_offset(11), (4, 1));
    // end of text
    assert_eq!(lex.str_offset(12), (4, 2));
}

#[test]
fn error_data_test() {
    // nothing added yet: C returns NULL
    assert_eq!(build_error_string(&[]), None);

    let items = [
        ErrorItem {
            file: Some(b"f.txt"),
            row: 1,
            column: 2,
            error: Some(b"bad"),
        },
        ErrorItem {
            file: Some(b"g.txt"),
            row: 10,
            column: 20,
            error: Some(b"worse"),
        },
    ];
    assert_eq!(
        build_error_string(&items).as_deref(),
        Some(&b"f.txt (1, 2): bad\ng.txt (10, 20): worse\n"[..])
    );
}

// ---- edge cases beyond the C test ----

#[test]
fn error_string_null_parts_print_null() {
    // glibc `%s` prints "(null)" for a NULL pointer; the shim follows.
    let items = [ErrorItem {
        file: None,
        row: u32::MAX,
        column: 0,
        error: None,
    }];
    assert_eq!(
        build_error_string(&items).as_deref(),
        Some(&b"(null) (4294967295, 0): (null)\n"[..])
    );
}

#[test]
fn compare_uses_platform_char_ordering() {
    // characterized, not endorsed: bytes >= 0x80 are `char`s, so they sort
    // below ASCII where `char` is signed (x86) and above where unsigned.
    let high_below = (0xC3u8 as c_char) < (b'a' as c_char);
    let expect = if high_below { -1 } else { 1 };
    assert_eq!(strref_cmp(sr(b"\xC3"), cs(b"a")), expect);
    assert_eq!(strref_cmpi(sr(b"\xC3"), cs(b"a")), expect);
    assert_eq!(strref_cmp_strref(sr(b"\xC3"), sr(b"a")), expect);
    assert_eq!(strref_cmpi_strref(sr(b"\xC3"), sr(b"a")), expect);
}

#[test]
fn cmpi_folds_ascii_only() {
    // 0xC3 0xA9 vs 0xC3 0x89: no folding of non-ASCII bytes
    assert_eq!(strref_cmpi(sr("é".as_bytes()), cs("É".as_bytes())), 1);
    // only a-z fold: '[' (0x5B) stays below '{' (0x7B)
    assert_eq!(strref_cmpi(sr(b"["), cs(b"{")), -1);
    assert_eq!(strref_cmpi(sr(b"a"), cs(b"B")), -1);
    assert_eq!(strref_cmpi(sr(b"b"), cs(b"A")), 1);
}

#[test]
fn leading_nul_strref_is_empty() {
    // strref_is_empty looks at the first byte too
    assert_eq!(strref_cmp(sr(b"\0abc"), cs(b"")), 0);
    assert_eq!(strref_cmp(sr(b"\0abc"), cs(b"a")), -1);
    assert_eq!(strref_cmp_strref(sr(b"\0a"), None), 0);
    assert_eq!(strref_cmp_strref(sr(b"a"), sr(b"\0a")), 1);
}

#[test]
fn embedded_nul_in_strref_is_compared_as_a_byte() {
    assert_eq!(strref_cmp_strref(sr(b"a\0b"), sr(b"a\0c")), -1);
    assert_eq!(strref_cmp_strref(sr(b"a\0b"), sr(b"a\0b")), 0);
    // against a C string, the C string's NUL ends the loop first, so the
    // bytes after the embedded NUL are never looked at (characterized, not
    // endorsed)
    assert_eq!(strref_cmp(sr(b"a\0b"), cs(b"a")), 0);
}

#[test]
fn nul_ends_c_strings_for_validators() {
    assert!(valid_int_str(cs(b"12\0x"), 0));
    assert!(valid_float_str(cs(b"1.5\0x"), 0));
}

#[test]
fn n_larger_than_string_stops_at_the_end() {
    assert!(valid_int_str(cs(b"12"), 99));
    assert!(valid_float_str(cs(b"1.5"), 99));
}

#[test]
fn non_ascii_bytes_are_single_other_tokens() {
    let src = "é1".as_bytes();
    let mut lex = Lexer::new(src);
    let ign = IgnoreWhitespace::Ignore;
    expect_token(&mut lex, src, ign, BaseTokenType::Other, &src[..1]);
    expect_token(&mut lex, src, ign, BaseTokenType::Other, &src[1..2]);
    expect_token(&mut lex, src, ign, BaseTokenType::Digit, b"1");
    expect_end(&mut lex, ign);
}

#[test]
fn text_ends_at_first_nul() {
    let mut lex = Lexer::new(b"ab\0cd");
    let ign = IgnoreWhitespace::Ignore;
    assert!(lex.get_base_token(ign).is_some());
    assert!(lex.get_base_token(ign).is_none());
}

#[test]
fn offset_inside_a_crlf_pair_counts_the_whole_pair() {
    // pointing at the '\n' of "\r\n": the C loop steps over both bytes
    // (characterized, not endorsed)
    let lex = Lexer::new(b"a\r\nb");
    assert_eq!(lex.str_offset(2), (2, 1));
}
