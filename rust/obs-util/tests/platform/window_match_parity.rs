//! Tier 3 parity: the `window_match` core and its shims against the original
//! `window-match.c.h` (oracle/window_match.c).
//!
//! The unchanged C test `test_window_match_steps` compiles the C into
//! itself. This file compares all three sides (oracle, shims, core)
//! exactly: the constants; the internal-exe, untitled-Explorer and class
//! tests over every listed name in every case pattern of its first three
//! letters, with every one-byte suffix and every one-byte cut, the generic
//! and UWP names at offsets 0 to 4 with non-ASCII bytes before, inside and
//! after them, and every byte value in every position of every listed name
//! (so the core's ASCII folding is C's `tolower()` in the C locale, over
//! all 255 bytes); the rating over every priority from -1 to 4, the four
//! flags and title results from -300 to 300 and at the ends of `int`; and
//! proptests.
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
    VALUE_NAMES, oracle_ms_window_match_is_generic_class, oracle_ms_window_match_is_internal_exe,
    oracle_ms_window_match_is_untitled_explorer, oracle_ms_window_match_is_uwp_class,
    oracle_ms_window_match_rating, oracle_window_match_value,
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
}
