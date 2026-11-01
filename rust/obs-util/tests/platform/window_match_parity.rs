//! Tier 3 parity: the `window_match` core and its shims against the original
//! `window-match.c.h` (oracle/window_match.c).
//!
//! The unchanged C test `test_window_match_steps` compiles the C into
//! itself, and `test_window_match_steps_rust` runs it against the shims on
//! Linux. This file compares all three sides (oracle, shims, core)
//! exactly: the constants; the internal-exe, untitled-Explorer and class
//! tests over every listed name in every case pattern of its first three
//! letters, with every one-byte suffix and every one-byte cut, the generic
//! and UWP names at offsets 0 to 4 with non-ASCII bytes before, inside and
//! after them, and every byte value in every position of every listed name
//! (so the core's ASCII folding is C's `tolower()` in the C locale, over
//! all 255 bytes); the rating over every priority from -1 to 4, the four
//! flags and title results from -300 to 300 and at the ends of `int`; the
//! string codec (encode, decode and split) over NULL, every string up to 5
//! bytes over the bytes it looks at (`#`, `:`, `2`, `3`, `A`, `a` and two
//! others), each encode at every buffer size from NULL and 0 to two past
//! the end, the whole decoded buffer (the bytes past the new NUL included)
//! and all six split outputs; and proptests, with the window list's
//! `title:class:exe` round trip through the shims.
//!
//! No intentional difference. Excluded, because the C leaves them
//! undefined: a title result of `INT_MIN` (`abs()`), and one whose distance
//! plus the penalty passes `INT_MAX`; `astrcmpi()` returns at most 255 in
//! magnitude. The Tier 1 tests pin what the core does with them. A null
//! class for `ms_window_match_is_uwp_class()`, which the C's `strcmp()`
//! dereferences, is checked on the Rust side only.

use std::ffi::{CString, c_char};
use std::ptr;

use obs_c_oracle::window_match::{
    VALUE_NAMES, oracle_ms_window_match_decode, oracle_ms_window_match_encode,
    oracle_ms_window_match_is_generic_class, oracle_ms_window_match_is_internal_exe,
    oracle_ms_window_match_is_untitled_explorer, oracle_ms_window_match_is_uwp_class,
    oracle_ms_window_match_rating, oracle_ms_window_match_split, oracle_window_match_value,
};
use obs_util::ffi::window_match as shim;
use obs_util::window_match::{
    self as core, EXE_PENALTY, EXPLORER_EXE, GENERIC_CLASS_SUBSTRINGS, INTERNAL_EXES_EXACT,
    INTERNAL_EXES_PARTIAL, NO_MATCH, PRIORITY_CLASS, PRIORITY_EXE, PRIORITY_TITLE, UWP_CLASSES,
};
use proptest::prelude::*;

/// `s` as a C string; `s` holds no NUL.
fn c(s: &[u8]) -> CString {
    CString::new(s).expect("no NUL in the built names")
}

/// Every listed exe, prefix and the Explorer name.
fn exe_names() -> Vec<&'static [u8]> {
    let mut names: Vec<&[u8]> = INTERNAL_EXES_EXACT.to_vec();
    names.extend(INTERNAL_EXES_PARTIAL);
    names.push(EXPLORER_EXE);
    names
}

/// Every listed class and class substring.
fn class_names() -> Vec<&'static [u8]> {
    let mut names: Vec<&[u8]> = GENERIC_CLASS_SUBSTRINGS.to_vec();
    names.extend(UWP_CLASSES);
    names
}

/// `s` with its first three letters in case pattern `bits` (bit i: byte i
/// flipped to the other case).
fn case_pattern(s: &[u8], bits: u8) -> Vec<u8> {
    s.iter()
        .enumerate()
        .map(|(i, &b)| {
            if i < 3 && bits & (1 << i) != 0 {
                if b.is_ascii_lowercase() {
                    b.to_ascii_uppercase()
                } else {
                    b.to_ascii_lowercase()
                }
            } else {
                b
            }
        })
        .collect()
}

/// Each name in every case pattern of its first three letters, with every
/// one-byte suffix, and with each byte cut.
fn built(names: &[&[u8]]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for name in names {
        for bits in 0..8 {
            let n = case_pattern(name, bits);
            for b in 1..=u8::MAX {
                out.push([&n[..], &[b]].concat());
            }
            for cut in 0..n.len() {
                let mut v = n.clone();
                v.remove(cut);
                out.push(v);
                out.push(n[..cut].to_vec());
            }
            out.push(n);
        }
    }
    out
}

/// Each name with each of its bytes replaced by every byte value.
fn every_byte(names: &[&[u8]]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for name in names {
        for pos in 0..name.len() {
            for b in 1..=u8::MAX {
                let mut v = name.to_vec();
                v[pos] = b;
                out.push(v);
            }
        }
    }
    out
}

fn check_exe(exe: &[u8]) {
    let s = c(exe);
    // SAFETY: a NUL-terminated string.
    let oracle = unsafe { oracle_ms_window_match_is_internal_exe(s.as_ptr()) };
    // SAFETY: a NUL-terminated string.
    let ffi = unsafe { shim::ms_window_match_is_internal_exe(s.as_ptr()) };
    let core = core::is_internal_exe(Some(exe));
    assert_eq!(
        (ffi, core),
        (oracle, oracle),
        "is_internal_exe {}",
        exe.escape_ascii()
    );
    for title_empty in [false, true] {
        // SAFETY: a NUL-terminated string.
        let oracle =
            unsafe { oracle_ms_window_match_is_untitled_explorer(s.as_ptr(), title_empty) };
        // SAFETY: a NUL-terminated string.
        let ffi = unsafe { shim::ms_window_match_is_untitled_explorer(s.as_ptr(), title_empty) };
        let core = core::is_untitled_explorer(Some(exe), title_empty);
        assert_eq!(
            (ffi, core),
            (oracle, oracle),
            "is_untitled_explorer {} {title_empty}",
            exe.escape_ascii()
        );
    }
}

fn check_class(class: &[u8]) {
    let s = c(class);
    // SAFETY: a NUL-terminated string.
    let oracle = unsafe { oracle_ms_window_match_is_generic_class(s.as_ptr()) };
    // SAFETY: a NUL-terminated string.
    let ffi = unsafe { shim::ms_window_match_is_generic_class(s.as_ptr()) };
    let core = core::is_generic_class(Some(class));
    assert_eq!(
        (ffi, core),
        (oracle, oracle),
        "is_generic_class {}",
        class.escape_ascii()
    );
    // SAFETY: a NUL-terminated string.
    let oracle = unsafe { oracle_ms_window_match_is_uwp_class(s.as_ptr()) };
    // SAFETY: a NUL-terminated string.
    let ffi = unsafe { shim::ms_window_match_is_uwp_class(s.as_ptr()) };
    let core = core::is_uwp_class(class);
    assert_eq!(
        (ffi, core),
        (oracle, oracle),
        "is_uwp_class {}",
        class.escape_ascii()
    );
}

fn check_rating(
    p: i32,
    uwp: bool,
    generic: bool,
    class_matches: bool,
    exe_matches: bool,
    cmp: i32,
) {
    // SAFETY: plain values.
    let oracle =
        unsafe { oracle_ms_window_match_rating(p, uwp, generic, class_matches, exe_matches, cmp) };
    let ffi = shim::ms_window_match_rating(p, uwp, generic, class_matches, exe_matches, cmp);
    let core = core::rating(p, uwp, generic, class_matches, exe_matches, cmp);
    assert_eq!(
        (ffi, core),
        (oracle, oracle),
        "rating({p}, {uwp}, {generic}, {class_matches}, {exe_matches}, {cmp})"
    );
}

/// Whether the C's rating is defined for this title result: not
/// `INT_MIN`, and the class penalty does not pass `INT_MAX`.
fn defined(cmp: i32) -> bool {
    cmp != i32::MIN
        && (cmp.unsigned_abs() <= (i32::MAX - EXE_PENALTY).unsigned_abs()
            || cmp.unsigned_abs() == i32::MAX.unsigned_abs())
}

#[test]
fn parity_constants() {
    let rust = [
        PRIORITY_CLASS,
        PRIORITY_TITLE,
        PRIORITY_EXE,
        NO_MATCH,
        EXE_PENALTY,
    ];
    for (i, (name, want)) in VALUE_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: plain values.
        assert_eq!(unsafe { oracle_window_match_value(i) }, want, "{name}");
    }
    // SAFETY: plain values.
    assert_eq!(unsafe { oracle_window_match_value(VALUE_NAMES.len()) }, -1);
}

#[test]
fn parity_exes_built() {
    for exe in built(&exe_names()) {
        check_exe(&exe);
    }
    for exe in [
        &b""[..],
        b"\xc3\xa9",
        b"windowsinternal\xc3\xa9",
        b"\xffexplorer.exe",
    ] {
        check_exe(exe);
    }
}

#[test]
fn parity_exes_every_byte() {
    for exe in every_byte(&exe_names()) {
        check_exe(&exe);
    }
}

#[test]
fn parity_exes_null() {
    // SAFETY: NULL is accepted by both.
    unsafe {
        assert_eq!(
            shim::ms_window_match_is_internal_exe(ptr::null()),
            oracle_ms_window_match_is_internal_exe(ptr::null())
        );
        for title_empty in [false, true] {
            assert_eq!(
                shim::ms_window_match_is_untitled_explorer(ptr::null(), title_empty),
                oracle_ms_window_match_is_untitled_explorer(ptr::null(), title_empty)
            );
        }
        assert_eq!(
            shim::ms_window_match_is_generic_class(ptr::null()),
            oracle_ms_window_match_is_generic_class(ptr::null())
        );
        // Rust side only: the C's strcmp() dereferences a null class.
        assert!(!shim::ms_window_match_is_uwp_class(ptr::null::<c_char>()));
    }
    assert!(!core::is_internal_exe(None));
    assert!(!core::is_untitled_explorer(None, true));
    assert!(!core::is_generic_class(None));
}

#[test]
fn parity_classes_built() {
    let names = class_names();
    for class in built(&names) {
        check_class(&class);
    }
    // at offsets 0 to 4, in each case pattern, with a non-ASCII byte before,
    // inside (at each position) and after
    for name in &names {
        for bits in 0..8 {
            let n = case_pattern(name, bits);
            for offset in 0..5 {
                for high in [0x80u8, 0xc3, 0xff] {
                    let lead = vec![b'q'; offset];
                    check_class(&[&lead[..], &[high], &n].concat());
                    check_class(&[&lead[..], &n, &[high]].concat());
                    for pos in 0..=n.len() {
                        let mut v = n.clone();
                        v.insert(pos, high);
                        check_class(&[&lead[..], &v].concat());
                    }
                }
            }
        }
    }
}

#[test]
fn parity_classes_every_byte() {
    for class in every_byte(&class_names()) {
        check_class(&class);
        check_class(&[b"x", &class[..], b"y"].concat());
    }
}

#[test]
fn parity_rating_grid() {
    let ends = [
        i32::MAX,
        i32::MIN + 1,
        i32::MAX - EXE_PENALTY,
        -(i32::MAX - EXE_PENALTY),
        255,
        -255,
    ];
    for p in -1..=4 {
        for flags in 0..16 {
            let (uwp, generic, class_matches, exe_matches) = (
                flags & 1 != 0,
                flags & 2 != 0,
                flags & 4 != 0,
                flags & 8 != 0,
            );
            for cmp in (-300..=300).chain(ends) {
                assert!(defined(cmp));
                check_rating(p, uwp, generic, class_matches, exe_matches, cmp);
            }
        }
    }
}

// ---- the string codec ----

type EncodeFn = unsafe extern "C" fn(*const c_char, *mut c_char, usize) -> usize;
type DecodeFn = unsafe extern "C" fn(*mut c_char) -> usize;
type SplitFn = unsafe extern "C" fn(*const c_char, *mut usize, *mut usize) -> bool;

/// `encode` of `s` (NULL for `None`) into a NULL buffer, then into each
/// size from 0 to two past the escaped length, the buffer full of `z`
/// first: the returns and the whole buffers.
fn encode_runs(encode: EncodeFn, s: Option<&CString>) -> Vec<(usize, Vec<u8>)> {
    let sp = s.map_or(ptr::null(), |s| s.as_ptr());
    // SAFETY: NULL or a C string; a NULL buffer is only measured.
    let need = unsafe { encode(sp, ptr::null_mut(), 0) };
    let mut runs = vec![(need, Vec::new())];
    for size in 0..need + 3 {
        let mut buf = vec![b'z'; size + 1];
        // SAFETY: `buf` has `size` writable bytes (one more as a guard).
        let got = unsafe { encode(sp, buf.as_mut_ptr().cast(), size) };
        runs.push((got, buf));
    }
    runs
}

/// `decode` of a copy of `s` (NULL for `None`): the return and the whole
/// buffer, its NUL included.
fn decode_run(decode: DecodeFn, s: Option<&CString>) -> (usize, Vec<u8>) {
    let Some(s) = s else {
        // SAFETY: NULL is allowed.
        return (unsafe { decode(ptr::null_mut()) }, Vec::new());
    };
    let mut buf = s.as_bytes_with_nul().to_vec();
    // SAFETY: a writable NUL-terminated copy.
    let got = unsafe { decode(buf.as_mut_ptr().cast()) };
    (got, buf)
}

/// `split` of `s` (NULL for `None`), the outputs full of 99 first: the
/// return and all six outputs.
fn split_run(split: SplitFn, s: Option<&CString>) -> (bool, [usize; 3], [usize; 3]) {
    let mut start = [99usize; 3];
    let mut len = [99usize; 3];
    let sp = s.map_or(ptr::null(), |s| s.as_ptr());
    // SAFETY: NULL or a C string, and three writable `size_t`s each.
    let got = unsafe { split(sp, start.as_mut_ptr(), len.as_mut_ptr()) };
    (got, start, len)
}

/// Oracle, shims and core agree on the codec of `s` (NULL for `None`).
fn check_codec(s: Option<&[u8]>) {
    let cs = s.map(c);
    let cs = cs.as_ref();
    let bytes = s.unwrap_or_default();

    let want = encode_runs(oracle_ms_window_match_encode, cs);
    assert_eq!(
        encode_runs(shim::ms_window_match_encode, cs),
        want,
        "encode({s:?})"
    );
    assert_eq!(core::encoded_len(bytes), want[0].0, "encode({s:?})");
    assert_eq!(core::encode(bytes).len(), want[0].0, "encode({s:?})");
    for (got, buf) in &want[1..] {
        let mut out = vec![b'z'; buf.len()];
        let size = buf.len() - 1;
        assert_eq!(
            core::encode_into(bytes, &mut out[..size]),
            *got,
            "encode({s:?}, {size})"
        );
        assert_eq!(&out, buf, "encode({s:?}, {size})");
    }

    let want = decode_run(oracle_ms_window_match_decode, cs);
    assert_eq!(
        decode_run(shim::ms_window_match_decode, cs),
        want,
        "decode({s:?})"
    );
    if s.is_some() {
        let mut buf = want.1.clone();
        let mut core_buf = cs
            .map(|s| s.as_bytes_with_nul().to_vec())
            .unwrap_or_default();
        let n = core_buf.len() - 1;
        assert_eq!(core::decode(&mut core_buf[..n]), want.0, "decode({s:?})");
        buf.truncate(core_buf.len());
        assert_eq!(core_buf, buf, "decode({s:?})");
    }

    let want = split_run(oracle_ms_window_match_split, cs);
    assert_eq!(
        split_run(shim::ms_window_match_split, cs),
        want,
        "split({s:?})"
    );
    let got = s
        .and_then(core::split)
        .map(|r| r.map(|r| (r.start, r.len())));
    let want = want.0.then(|| [0, 1, 2].map(|i| (want.1[i], want.2[i])));
    assert_eq!(got, want, "split({s:?})");
}

/// The window list's `title:class:exe` of `parts` through the shims, split
/// and decoded back, gives each part.
fn check_round_trip(parts: [&[u8]; 3]) {
    let mut joined = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            joined.push(b':');
        }
        let part = c(part);
        // SAFETY: a C string, measured with a NULL buffer.
        let need = unsafe { shim::ms_window_match_encode(part.as_ptr(), ptr::null_mut(), 0) };
        let mut buf = vec![0u8; need + 1];
        // SAFETY: `buf` has `need + 1` writable bytes.
        let got = unsafe {
            shim::ms_window_match_encode(part.as_ptr(), buf.as_mut_ptr().cast(), need + 1)
        };
        assert_eq!(got, need);
        joined.extend_from_slice(&buf[..need]);
    }
    let joined = c(&joined);
    let (ok, start, len) = split_run(shim::ms_window_match_split, Some(&joined));
    assert!(ok, "{joined:?}");
    for (i, want) in parts.iter().enumerate() {
        let field = c(&joined.as_bytes()[start[i]..start[i] + len[i]]);
        let (n, buf) = decode_run(shim::ms_window_match_decode, Some(&field));
        assert_eq!(&buf[..n], *want, "round trip {joined:?}");
    }
}

/// The bytes the codec looks at, and two it doesn't.
const CODEC_BYTES: &[u8] = b"#:23Aa\xc3\x01";

#[test]
fn parity_codec_edges() {
    check_codec(None);
    for s in [
        &b""[..],
        b":",
        b"::",
        b":::",
        b"a:b:c",
        b"a:b:c:d",
        b"a:b",
        b"#",
        b"#22",
        b"#3A",
        b"#223A",
        b"##3A22",
        b"x#3a",
        b"#2222",
        b"#3A3A",
        b"#3#3AA",
        b"Title: x # y:Class:app.exe",
        b"#3A#22#3A#22:#22#3A:#",
        b"\xe2\x80\x94:\xff:\x01",
    ] {
        check_codec(Some(s));
    }
}

/// Every string up to 5 bytes over [`CODEC_BYTES`].
#[test]
fn parity_codec_grid() {
    let n = CODEC_BYTES.len();
    let mut s = Vec::new();
    let mut item = 0u64;
    for width in 0..=5u32 {
        for mut k in 0..n.pow(width) {
            item += 1;
            if width > 3 && !obs_c_oracle::parity::keep(item) {
                continue;
            }
            s.clear();
            for _ in 0..width {
                s.push(CODEC_BYTES[k % n]);
                k /= n;
            }
            check_codec(Some(&s));
        }
    }
}

#[test]
fn parity_codec_round_trip_edges() {
    let parts: [&[u8]; 8] = [
        b"",
        b"#",
        b":",
        b"#:",
        b"#22",
        b"#3A",
        b"x#3a",
        b"\xc3\xa9#:\xff",
    ];
    for t in parts {
        for cl in parts {
            for e in parts {
                check_round_trip([t, cl, e]);
            }
        }
    }
}

/// A string rich in the bytes the codec looks at, with any other byte but
/// NUL.
fn codec_string() -> impl Strategy<Value = Vec<u8>> {
    proptest::collection::vec(
        prop_oneof![
            4 => proptest::sample::select(b"#:23A".to_vec()),
            1 => 1u8..=u8::MAX,
        ],
        0..40,
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(4096)))]

    #[test]
    fn parity_prop_exe(exe in proptest::collection::vec(1u8..=u8::MAX, 0..40)) {
        check_exe(&exe);
    }

    #[test]
    fn parity_prop_class(class in proptest::collection::vec(1u8..=u8::MAX, 0..40)) {
        check_class(&class);
    }

    #[test]
    fn parity_prop_listed(
        which in 0usize..20,
        bits in 0u8..8,
        lead in proptest::collection::vec(1u8..=u8::MAX, 0..4),
        tail in proptest::collection::vec(1u8..=u8::MAX, 0..4),
    ) {
        let mut names = exe_names();
        names.extend(class_names());
        let name = case_pattern(names[which % names.len()], bits);
        let s = [&lead[..], &name, &tail[..]].concat();
        check_exe(&s);
        check_class(&s);
    }

    #[test]
    fn parity_prop_rating(
        p in any::<i32>(),
        uwp: bool,
        generic: bool,
        class_matches: bool,
        exe_matches: bool,
        cmp in any::<i32>().prop_filter("defined in C", |&c| defined(c)),
    ) {
        check_rating(p, uwp, generic, class_matches, exe_matches, cmp);
    }

    #[test]
    fn parity_prop_codec(s in codec_string()) {
        check_codec(Some(&s));
    }

    #[test]
    fn parity_prop_codec_round_trip(
        title in codec_string(),
        class in codec_string(),
        exe in codec_string(),
    ) {
        check_round_trip([&title, &class, &exe]);
    }
}
