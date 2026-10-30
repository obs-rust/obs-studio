//! Tier 3: the Rust C ABI shim (and through it the safe core) behaves
//! exactly like the original `util/cf-tokenizer.c`, compiled as an oracle
//! on top of the lexer oracle.
//!
//! `cf_lexer_lex` is compared field by field: the return value, `file`,
//! `unexpected_eof`, the base lexer offset, the reformatted text up to
//! `write_offset`, and every token (type, `lex` back-pointer, and `str` /
//! `unmerged_str` as offsets into their buffers), plus the darray's `num`
//! and `capacity`.
//!
//! Intentional differences, excluded here:
//! - Text ending in a line splice inside a comment or string makes C read
//!   past the terminator; inputs never end in a splice.
//! - `cf_literal_to_str(NULL, ...)` returns NULL (C dereferences it).
//!   `\x` and octal escapes can make C read past the literal's NUL; the
//!   literals here are followed by NUL padding, so C reads NUL there like
//!   Rust does.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use core::ffi::{CStr, c_char, c_void};
use core::ptr;

use obs_c_oracle::cf_tokenizer as c;
use obs_util::ffi::cf_tokenizer as rs;
use obs_util::ffi::darray::bfree;
use proptest::prelude::*;

const SOURCE_PIECES: &[&[u8]] = &[
    b"a",
    b"_x",
    b"int",
    b"9",
    b"1.5",
    b".",
    b"e",
    b" ",
    b"\t",
    b"  ",
    b"\n",
    b"\r\n",
    b"\n\r",
    b"\r",
    b"\\\n",
    b"\\\r\n",
    b"\\",
    b"/",
    b"*",
    b"//",
    b"/*",
    b"*/",
    b"\"",
    b"'",
    b"<",
    b">",
    b"#",
    b"include",
    b"import",
    b"#include ",
    b"\\\"",
    b"\\'",
    b"+",
    b";",
    b"(",
    b")",
    b",",
    b"\xc3",
];

const LITERAL_PIECES: &[&[u8]] = &[
    b"a",
    b"\\n",
    b"\\t",
    b"\\0",
    b"\\\\",
    b"\\'",
    b"\\\"",
    b"\\?",
    b"\\x",
    b"\\x4",
    b"\\x41",
    b"\\x0x7f",
    b"\\X-1",
    b"\\x123456789a",
    b"\\1",
    b"\\12",
    b"\\101",
    b"\\777",
    b"\\8",
    b"\\q",
    b"\\",
    b" ",
    b"\xc3",
    b"7",
    b"f",
];

/// Pieces of C-like source, weighted toward what the tokenizer treats
/// specially.
fn source() -> impl Strategy<Value = Vec<u8>> {
    let piece = prop_oneof![
        3 => prop::sample::select(SOURCE_PIECES).prop_map(<[u8]>::to_vec),
        1 => any::<u8>().prop_filter("no NUL", |&b| b != 0).prop_map(|b| vec![b]),
    ];
    prop::collection::vec(piece, 1..40).prop_map(|p| {
        let mut v = p.concat();
        // Never end in a line splice: C would read past the NUL.
        if v.ends_with(b"\\\n")
            || v.ends_with(b"\\\r")
            || v.ends_with(b"\\\r\n")
            || v.ends_with(b"\\\n\r")
        {
            v.push(b'x');
        }
        v
    })
}

fn nul_terminated(v: &[u8]) -> Vec<u8> {
    let mut v = v.to_vec();
    v.push(0);
    v
}

/// What a lexer holds after `cf_lexer_lex`, as comparable values.
#[derive(Debug, PartialEq)]
struct Snapshot {
    ok: bool,
    file: Option<Vec<u8>>,
    unexpected_eof: bool,
    offset: Option<usize>,
    reformatted: Option<Vec<u8>>,
    tokens: Vec<(i32, bool, usize, usize, usize, usize)>,
    num: usize,
    capacity: usize,
}

/// # Safety
///
/// The pointers must describe a lexer filled by `cf_lexer_lex` (or freed).
#[allow(clippy::too_many_arguments)]
unsafe fn snapshot(
    ok: bool,
    lex_ptr: *const c_void,
    file: *const c_char,
    text: *const c_char,
    offset: *const c_char,
    reformatted: *const c_char,
    write_offset: *const c_char,
    tokens: &[(
        *const c_void,
        *const c_char,
        usize,
        *const c_char,
        usize,
        i32,
    )],
    num: usize,
    capacity: usize,
    unexpected_eof: bool,
) -> Snapshot {
    // SAFETY: per the contract.
    unsafe {
        let s = |p: *const c_char| (!p.is_null()).then(|| CStr::from_ptr(p).to_bytes().to_vec());
        let rel = |p: *const c_char, base: *const c_char| (p as usize).wrapping_sub(base as usize);
        let reformatted_bytes = (!reformatted.is_null()).then(|| {
            let n = rel(write_offset, reformatted);
            core::slice::from_raw_parts(reformatted.cast::<u8>(), n + 1).to_vec()
        });
        Snapshot {
            ok,
            file: s(file),
            unexpected_eof,
            offset: (!text.is_null()).then(|| rel(offset, text)),
            reformatted: reformatted_bytes,
            tokens: tokens
                .iter()
                .map(|&(lex, str_array, str_len, unmerged, unmerged_len, kind)| {
                    (
                        kind,
                        lex == lex_ptr,
                        rel(str_array, reformatted),
                        str_len,
                        rel(unmerged, text),
                        unmerged_len,
                    )
                })
                .collect(),
            num,
            capacity,
        }
    }
}

/// # Safety
///
/// `array` must be NULL with `num` 0, or hold `num` items of `T`.
unsafe fn items<'a, T>(array: *mut c_void, num: usize) -> &'a [T] {
    if array.is_null() {
        &[]
    } else {
        // SAFETY: per the contract.
        unsafe { core::slice::from_raw_parts(array.cast::<T>(), num) }
    }
}

/// Lexes `input` (NULL when `None`) with both implementations, on lexers
/// that already hold `previous`, and returns both snapshots.
fn lex_both(
    previous: Option<&[u8]>,
    input: Option<&[u8]>,
    file: Option<&[u8]>,
) -> (Snapshot, Snapshot) {
    let input = input.map(nul_terminated);
    let file = file.map(nul_terminated);
    let previous = previous.map(nul_terminated);
    let p = |v: &Option<Vec<u8>>| {
        v.as_ref()
            .map_or(ptr::null(), |v| v.as_ptr().cast::<c_char>())
    };

    // SAFETY: both lexers are initialized before use and freed after; all
    // strings are NUL-terminated and outlive the calls.
    unsafe {
        let mut r = core::mem::MaybeUninit::<rs::cf_lexer>::uninit();
        let mut o = core::mem::MaybeUninit::<c::OracleCfLexer>::uninit();
        rs::cf_lexer_init(r.as_mut_ptr());
        c::oracle_cf_lexer_init(o.as_mut_ptr());
        let (r, o) = (r.assume_init_mut(), o.assume_init_mut());

        if previous.is_some() {
            rs::cf_lexer_lex(r, p(&previous), ptr::null());
            c::oracle_cf_lexer_lex(o, p(&previous), ptr::null());
        }
        let r_ok = rs::cf_lexer_lex(r, p(&input), p(&file));
        let o_ok = c::oracle_cf_lexer_lex(o, p(&input), p(&file));

        let rt: Vec<_> = items::<rs::cf_token>(r.tokens.array, r.tokens.num)
            .iter()
            .map(|t| {
                (
                    t.lex.cast::<c_void>(),
                    t.str.array,
                    t.str.len,
                    t.unmerged_str.array,
                    t.unmerged_str.len,
                    t.r#type,
                )
            })
            .collect();
        let ot: Vec<_> = items::<c::OracleCfToken>(o.tokens.array, o.tokens.num)
            .iter()
            .map(|t| {
                (
                    t.lex.cast::<c_void>(),
                    t.str.array,
                    t.str.len,
                    t.unmerged_str.array,
                    t.unmerged_str.len,
                    t.kind,
                )
            })
            .collect();

        let rs_snap = snapshot(
            r_ok,
            ptr::from_ref(r).cast(),
            r.file,
            r.base_lexer.text,
            r.base_lexer.offset,
            r.reformatted,
            r.write_offset,
            &rt,
            r.tokens.num,
            r.tokens.capacity,
            r.unexpected_eof,
        );
        let c_snap = snapshot(
            o_ok,
            ptr::from_ref(o).cast(),
            o.file,
            o.base_lexer.text,
            o.base_lexer.offset,
            o.reformatted,
            o.write_offset,
            &ot,
            o.tokens.num,
            o.tokens.capacity,
            o.unexpected_eof,
        );

        rs::cf_lexer_free(r);
        c::oracle_cf_lexer_free(o);
        assert!(r.file.is_null() && r.reformatted.is_null() && r.tokens.array.is_null());
        assert!(o.file.is_null() && o.reformatted.is_null() && o.tokens.array.is_null());
        (rs_snap, c_snap)
    }
}

/// A quoted literal with escapes, an optional tail, and the count to pass.
fn literal() -> impl Strategy<Value = (Vec<u8>, usize)> {
    let body_piece = prop_oneof![
        3 => prop::sample::select(LITERAL_PIECES).prop_map(<[u8]>::to_vec),
        1 => any::<u8>().prop_filter("no NUL", |&b| b != 0).prop_map(|b| vec![b]),
    ];
    (
        prop::sample::select(&b"\"'x"[..]),
        prop::collection::vec(body_piece, 0..8),
        prop::sample::select(&b"\"'x"[..]),
        prop::collection::vec(any::<u8>().prop_filter("no NUL", |&b| b != 0), 0..4),
        any::<prop::sample::Index>(),
        any::<bool>(),
    )
        .prop_map(|(open, body, close, tail, at, explicit)| {
            let mut v = vec![open];
            v.extend(body.concat());
            v.push(close);
            let lit_len = v.len();
            v.extend(tail);
            // count: 0 (strlen), the literal's length, or anything up to
            // the whole text
            let count = if explicit {
                at.index(v.len() + 1)
            } else if at.index(2) == 0 {
                0
            } else {
                lit_len
            };
            (v, count)
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(4096)))]

    #[test]
    fn cf_lexer_lex_matches_c(input in source(), file in prop::option::of("[a-z./]{0,8}")) {
        let (ours, theirs) = lex_both(None, Some(&input), file.as_deref().map(str::as_bytes));
        prop_assert_eq!(ours, theirs);
    }

    #[test]
    fn cf_lexer_relex_matches_c(first in source(), second in source()) {
        let (ours, theirs) = lex_both(Some(&first), Some(&second), Some(b"f.c"));
        prop_assert_eq!(ours, theirs);
    }

    #[test]
    fn cf_literal_to_str_matches_c((text, count) in literal()) {
        // NUL padding: escapes may make C read past the text's NUL.
        let mut buf = text.clone();
        buf.extend([0u8; 32]);
        let p = buf.as_ptr().cast::<c_char>();
        // SAFETY: `buf` is readable for `count` bytes and NUL-padded after.
        unsafe {
            let ours = rs::cf_literal_to_str(p, count);
            let theirs = c::oracle_cf_literal_to_str(p, count);
            prop_assert_eq!(ours.is_null(), theirs.is_null());
            if !ours.is_null() {
                let n = if count == 0 { text.len() } else { count };
                let a = core::slice::from_raw_parts(ours.cast::<u8>(), n - 1);
                let b = core::slice::from_raw_parts(theirs.cast::<u8>(), n - 1);
                prop_assert_eq!(a, b);
            }
            bfree(ours.cast());
            bfree(theirs.cast());
        }
    }
}

#[test]
fn null_and_empty_input() {
    for previous in [None, Some(&b"int x;"[..])] {
        let (ours, theirs) = lex_both(previous, None, Some(b"t.c"));
        assert_eq!(ours, theirs);
        assert!(!ours.ok && ours.tokens.is_empty() && ours.file.is_none());

        let (ours, theirs) = lex_both(previous, Some(b""), Some(b"t.c"));
        assert_eq!(ours, theirs);
        assert!(!ours.ok && ours.tokens.is_empty());
    }
}

#[test]
fn literal_null_returns_null() {
    // Intentional difference: C dereferences NULL.
    // SAFETY: NULL is checked first.
    assert!(unsafe { rs::cf_literal_to_str(ptr::null(), 0) }.is_null());
}
