//! Tier 3: the Rust C ABI shims behave exactly like the original
//! `libobs/util/lexer.c`, compiled as an oracle (`oracle_*` symbols; its
//! `dstr_catf` is the dstr oracle's). Inputs are arbitrary byte strings,
//! biased toward the interesting characters (letters, digits, signs, `.`,
//! `e`, whitespace, CR/LF, NUL) and including non-ASCII bytes and NULs
//! inside strrefs.
//!
//! Mutation check: each property below fails if the matching part of the
//! core is mutated, e.g. `<=` -> `<` in `cmp_strref_c`'s loop bound, an
//! unsigned instead of `c_char` comparison, dropping the `+= 1` for a CRLF
//! pair in `scan_base_token`, or `n -= 1` -> `n -= 2` in `valid_int_str`.
//!
//! Intentional differences from C (not generated here): a NULL
//! `lex->text` with a non-NULL `str` crashes in C and the shim alike.

use core::ffi::{CStr, c_char, c_int, c_void};
use std::ffi::CString;

use obs_c_oracle::lexer as c;
use obs_util::ffi::darray::{bfree, darray};
use obs_util::ffi::lexer as rs;
use proptest::prelude::*;

const INTERESTING: &[u8] = b"aAbzZ09 \t\r\n+-.eE_\0";
const NUMERIC: &[u8] = b"0123456789+-.eE x";

fn text_byte() -> impl Strategy<Value = u8> {
    prop_oneof![
        4 => proptest::sample::select(INTERESTING),
        1 => any::<u8>(),
    ]
}

fn text_bytes(max: usize) -> impl Strategy<Value = Vec<u8>> {
    proptest::collection::vec(text_byte(), 0..=max)
}

fn num_bytes(max: usize) -> impl Strategy<Value = Vec<u8>> {
    proptest::collection::vec(
        prop_oneof![
            6 => proptest::sample::select(NUMERIC),
            1 => any::<u8>(),
        ],
        0..=max,
    )
}

/// A C string: cut at the first NUL.
fn cstring(bytes: &[u8]) -> CString {
    let cut: Vec<u8> = bytes.iter().copied().take_while(|&b| b != 0).collect();
    CString::new(cut).unwrap()
}

#[derive(Debug, Clone)]
enum RefSpec {
    NullPtr,
    NullArray(usize),
    Seg(Vec<u8>, prop::sample::Index),
}

fn ref_spec() -> impl Strategy<Value = RefSpec> {
    prop_oneof![
        1 => Just(RefSpec::NullPtr),
        1 => (0..4usize).prop_map(RefSpec::NullArray),
        8 => (text_bytes(12), any::<prop::sample::Index>())
            .prop_map(|(v, i)| RefSpec::Seg(v, i)),
    ]
}

/// The same strref for both implementations.
struct Built {
    _buf: Vec<u8>,
    null_ptr: bool,
    ours: rs::strref,
    theirs: c::OracleStrref,
}

impl Built {
    fn new(spec: &RefSpec) -> Self {
        let (mut buf, null_ptr, array_null, len) = match spec {
            RefSpec::NullPtr => (Vec::new(), true, true, 0),
            RefSpec::NullArray(len) => (Vec::new(), false, true, *len),
            RefSpec::Seg(v, i) => (v.clone(), false, false, i.index(v.len() + 1)),
        };
        // keep the pointer valid even for an empty segment
        buf.push(0);
        let array = if array_null {
            core::ptr::null()
        } else {
            buf.as_ptr().cast::<c_char>()
        };
        Built {
            _buf: buf,
            null_ptr,
            ours: rs::strref { array, len },
            theirs: c::OracleStrref { array, len },
        }
    }

    fn o(&self) -> *const rs::strref {
        if self.null_ptr {
            core::ptr::null()
        } else {
            &self.ours
        }
    }

    fn t(&self) -> *const c::OracleStrref {
        if self.null_ptr {
            core::ptr::null()
        } else {
            &self.theirs
        }
    }
}

fn opt_ptr(s: &Option<CString>) -> *const c_char {
    s.as_ref().map_or(core::ptr::null(), |s| s.as_ptr())
}

proptest! {
    #[test]
    fn strref_comparisons_match_c(
        a in ref_spec(),
        b in ref_spec(),
        s in proptest::option::of(text_bytes(12)),
    ) {
        let (a, b) = (Built::new(&a), Built::new(&b));
        let s = s.map(|s| cstring(&s));
        let sp = opt_ptr(&s);
        // SAFETY: all pointers are null or point at live, valid data.
        unsafe {
            prop_assert_eq!(rs::strref_cmp(a.o(), sp), c::oracle_strref_cmp(a.t(), sp));
            prop_assert_eq!(rs::strref_cmpi(a.o(), sp), c::oracle_strref_cmpi(a.t(), sp));
            prop_assert_eq!(
                rs::strref_cmp_strref(a.o(), b.o()),
                c::oracle_strref_cmp_strref(a.t(), b.t())
            );
            prop_assert_eq!(
                rs::strref_cmpi_strref(a.o(), b.o()),
                c::oracle_strref_cmpi_strref(a.t(), b.t())
            );
        }
    }

    /// Same, over a small alphabet so equal and near-equal strings are common.
    #[test]
    fn strref_comparisons_match_c_near_equal(
        a in proptest::collection::vec(proptest::sample::select(&b"aAbB\0\xC3\xA9"[..]), 0..6),
        b in proptest::collection::vec(proptest::sample::select(&b"aAbB\0\xC3\xA9"[..]), 0..6),
        la in any::<prop::sample::Index>(),
        lb in any::<prop::sample::Index>(),
    ) {
        let a = Built::new(&RefSpec::Seg(a.clone(), la));
        let b = Built::new(&RefSpec::Seg(b.clone(), lb));
        let cs = cstring(&b._buf);
        // SAFETY: live, valid data.
        unsafe {
            prop_assert_eq!(
                rs::strref_cmp_strref(a.o(), b.o()),
                c::oracle_strref_cmp_strref(a.t(), b.t())
            );
            prop_assert_eq!(
                rs::strref_cmpi_strref(a.o(), b.o()),
                c::oracle_strref_cmpi_strref(a.t(), b.t())
            );
            prop_assert_eq!(
                rs::strref_cmp(a.o(), cs.as_ptr()),
                c::oracle_strref_cmp(a.t(), cs.as_ptr())
            );
            prop_assert_eq!(
                rs::strref_cmpi(a.o(), cs.as_ptr()),
                c::oracle_strref_cmpi(a.t(), cs.as_ptr())
            );
        }
    }

    #[test]
    fn validators_match_c(
        s in proptest::option::of(prop_oneof![num_bytes(10), text_bytes(10)]),
        n in prop_oneof![Just(0usize), 1..6usize, Just(100usize)],
    ) {
        let s = s.map(|s| cstring(&s));
        let sp = opt_ptr(&s);
        // SAFETY: null or a live C string.
        unsafe {
            prop_assert_eq!(rs::valid_int_str(sp, n), c::oracle_valid_int_str(sp, n));
            prop_assert_eq!(rs::valid_float_str(sp, n), c::oracle_valid_float_str(sp, n));
        }
    }

    /// Arbitrary `iws` values (C only tests `== IGNORE_WHITESPACE`) from any
    /// starting offset, checking every step's result, token and cursor.
    #[test]
    fn getbasetoken_steps_match_c(
        text in text_bytes(24),
        start in any::<prop::sample::Index>(),
        iws_seq in proptest::collection::vec(-1..=2 as c_int, 0..=30),
    ) {
        let text = cstring(&text);
        let base = text.as_ptr();
        let start = start.index(text.as_bytes().len() + 1);
        // SAFETY: `start <= len`, so the offset stays within the string.
        let off = unsafe { base.add(start) };
        let mut ours = rs::lexer { text: base.cast_mut(), offset: off };
        let mut theirs = c::OracleLexer { text: base.cast_mut(), offset: off };
        for (step, &iws) in iws_seq.iter().enumerate() {
            let (a, b, t1, t2) = step_both(&mut ours, &mut theirs, iws);
            prop_assert_eq!(a, b, "result at step {}", step);
            prop_assert_eq!(ours.offset, theirs.offset, "offset at step {}", step);
            prop_assert_eq!(t1.text.array, t2.text.array, "array at step {}", step);
            prop_assert_eq!(t1.text.len, t2.text.len);
            prop_assert_eq!(t1.r#type, t2.r#type);
            prop_assert_eq!(t1.passed_whitespace, t2.passed_whitespace);
        }
    }

    /// A full walk to the end of the text, whitespace parsed or ignored.
    #[test]
    fn getbasetoken_full_walk_matches_c(
        text in text_bytes(40),
        ignore in any::<bool>(),
    ) {
        let text = cstring(&text);
        let base = text.as_ptr();
        let mut ours = rs::lexer { text: base.cast_mut(), offset: base };
        let mut theirs = c::OracleLexer { text: base.cast_mut(), offset: base };
        let iws = c_int::from(ignore);
        let mut misses = 0;
        for step in 0..=text.as_bytes().len() + 2 {
            let (a, b, t1, t2) = step_both(&mut ours, &mut theirs, iws);
            prop_assert_eq!(a, b, "result at step {}", step);
            prop_assert_eq!(ours.offset, theirs.offset, "offset at step {}", step);
            prop_assert_eq!(t1.text.array, t2.text.array);
            prop_assert_eq!(t1.text.len, t2.text.len);
            prop_assert_eq!(t1.r#type, t2.r#type);
            if !a {
                misses += 1;
                if misses == 2 {
                    break;
                }
            }
        }
    }

    /// Row/column for every `str` in the text, for text starting anywhere in
    /// the buffer (so `str` may precede the text, which gives (1, 1)).
    /// Interior NULs are plain columns in C and here.
    #[test]
    fn getstroffset_matches_c(
        text in text_bytes(24),
        text_start in any::<prop::sample::Index>(),
    ) {
        let mut buf = text;
        buf.push(0);
        let len = buf.len() - 1;
        let base = buf.as_ptr().cast::<c_char>();
        let k = text_start.index(len + 1);
        // SAFETY: `k <= len`, inside the buffer.
        let text_ptr = unsafe { base.add(k) }.cast_mut();
        let ours = rs::lexer { text: text_ptr, offset: text_ptr };
        let theirs = c::OracleLexer { text: text_ptr, offset: text_ptr };
        for p in 0..=len {
            let (mut r1, mut c1, mut r2, mut c2) = (7u32, 8u32, 7u32, 8u32);
            // SAFETY: `p <= len`; bytes up to the one after `str` are in the buffer.
            unsafe {
                let s = base.add(p);
                rs::lexer_getstroffset(&ours, s, &mut r1, &mut c1);
                c::oracle_lexer_getstroffset(&theirs, s, &mut r2, &mut c2);
            }
            prop_assert_eq!((r1, c1), (r2, c2), "str at {} (text at {})", p, k);
        }
    }
}

/// One `lexer_getbasetoken` call on both lexers, with sentinel-filled tokens.
fn step_both(
    ours: &mut rs::lexer,
    theirs: &mut c::OracleLexer,
    iws: c_int,
) -> (bool, bool, rs::base_token, c::OracleBaseToken) {
    let mut t1 = rs::base_token {
        text: rs::strref {
            array: core::ptr::null(),
            len: 77,
        },
        r#type: 99,
        passed_whitespace: true,
    };
    let mut t2 = c::OracleBaseToken {
        text: c::OracleStrref {
            array: core::ptr::null(),
            len: 77,
        },
        r#type: 99,
        passed_whitespace: true,
    };
    // SAFETY: both lexers point into a live NUL-terminated string.
    let (a, b) = unsafe {
        (
            rs::lexer_getbasetoken(ours, &mut t1, iws),
            c::oracle_lexer_getbasetoken(theirs, &mut t2, iws),
        )
    };
    (a, b, t1, t2)
}

#[test]
fn getbasetoken_null_offset_matches_c() {
    let mut ours = rs::lexer {
        text: core::ptr::null_mut(),
        offset: core::ptr::null(),
    };
    let mut theirs = c::OracleLexer {
        text: core::ptr::null_mut(),
        offset: core::ptr::null(),
    };
    for iws in [0, 1] {
        let (a, b, t1, t2) = step_both(&mut ours, &mut theirs, iws);
        assert!(!a && !b);
        assert!(ours.offset.is_null() && theirs.offset.is_null());
        assert_eq!(t1.r#type, t2.r#type);
        assert_eq!(t1.text.len, t2.text.len);
    }
}

#[test]
fn getstroffset_null_str_leaves_outputs_alone() {
    let text = CString::new("ab\ncd").unwrap();
    let ours = rs::lexer {
        text: text.as_ptr().cast_mut(),
        offset: text.as_ptr(),
    };
    let theirs = c::OracleLexer {
        text: text.as_ptr().cast_mut(),
        offset: text.as_ptr(),
    };
    let (mut r1, mut c1, mut r2, mut c2) = (99u32, 98u32, 99u32, 98u32);
    // SAFETY: valid lexers; NULL `str` returns before touching the outputs.
    unsafe {
        rs::lexer_getstroffset(&ours, core::ptr::null(), &mut r1, &mut c1);
        c::oracle_lexer_getstroffset(&theirs, core::ptr::null(), &mut r2, &mut c2);
    }
    assert_eq!((r1, c1), (99, 98));
    assert_eq!((r2, c2), (99, 98));
}

#[derive(Debug, Clone)]
struct Add {
    file: Option<Vec<u8>>,
    row: u32,
    col: u32,
    msg: Option<Vec<u8>>,
    level: c_int,
    null_data: bool,
}

fn add_spec() -> impl Strategy<Value = Add> {
    (
        proptest::option::weighted(0.9, text_bytes(8)),
        any::<u32>(),
        any::<u32>(),
        proptest::option::weighted(0.9, text_bytes(10)),
        -1..=2 as c_int,
        prop::bool::weighted(0.1),
    )
        .prop_map(|(file, row, col, msg, level, null_data)| Add {
            file,
            row,
            col,
            msg,
            level,
            null_data,
        })
}

/// Reads a possibly-NULL C string.
///
/// # Safety
///
/// `p` is null or a C string.
unsafe fn bytes_of<'a>(p: *const c_char) -> Option<&'a [u8]> {
    if p.is_null() {
        None
    } else {
        // SAFETY: per the contract.
        Some(unsafe { CStr::from_ptr(p) }.to_bytes())
    }
}

proptest! {
    /// Several `error_data_add`s (including to a NULL `error_data`), then
    /// `error_data_buildstring`; the arrays must grow identically too.
    #[test]
    fn error_data_matches_c(adds in proptest::collection::vec(add_spec(), 0..=9)) {
        let files: Vec<Option<CString>> =
            adds.iter().map(|a| a.file.as_deref().map(cstring)).collect();
        let msgs: Vec<Option<CString>> =
            adds.iter().map(|a| a.msg.as_deref().map(cstring)).collect();
        let mut ours = rs::error_data {
            errors: darray { array: core::ptr::null_mut(), num: 0, capacity: 0 },
        };
        let mut theirs = c::OracleErrorData {
            errors: c::OracleErrorArray { array: core::ptr::null_mut(), num: 0, capacity: 0 },
        };

        // SAFETY: the file/msg strings outlive the data; items are freed below.
        unsafe {
            for (i, a) in adds.iter().enumerate() {
                let (f, m) = (opt_ptr(&files[i]), opt_ptr(&msgs[i]));
                if a.null_data {
                    rs::error_data_add(core::ptr::null_mut(), f, a.row, a.col, m, a.level);
                    c::oracle_error_data_add(core::ptr::null_mut(), f, a.row, a.col, m, a.level);
                } else {
                    rs::error_data_add(&mut ours, f, a.row, a.col, m, a.level);
                    c::oracle_error_data_add(&mut theirs, f, a.row, a.col, m, a.level);
                }
                prop_assert_eq!(ours.errors.num, theirs.errors.num);
                prop_assert_eq!(ours.errors.capacity, theirs.errors.capacity, "capacity after add {}", i);
            }

            let num = ours.errors.num;
            for i in 0..num {
                let a = *ours.errors.array.cast::<rs::error_item>().add(i);
                let b = *theirs.errors.array.cast::<c::OracleErrorItem>().add(i);
                prop_assert_eq!(a.file, b.file);
                prop_assert_eq!((a.row, a.column, a.level), (b.row, b.column, b.level));
                prop_assert_eq!(bytes_of(a.error), bytes_of(b.error), "error text {}", i);
            }

            let s1 = rs::error_data_buildstring(&mut ours);
            let s2 = c::oracle_error_data_buildstring(&mut theirs);
            prop_assert_eq!(s1.is_null(), s2.is_null());
            prop_assert_eq!(bytes_of(s1), bytes_of(s2));
            bfree(s1.cast::<c_void>());
            bfree(s2.cast::<c_void>());

            // error_data_free from lexer.h
            for i in 0..num {
                bfree((*ours.errors.array.cast::<rs::error_item>().add(i)).error.cast::<c_void>());
                bfree((*theirs.errors.array.cast::<c::OracleErrorItem>().add(i)).error.cast::<c_void>());
            }
            bfree(ours.errors.array);
            bfree(theirs.errors.array);
        }
    }
}
