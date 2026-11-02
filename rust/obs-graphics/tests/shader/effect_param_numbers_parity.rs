//! Tier 3: the Rust shims and the core read effect numbers exactly like the
//! original `libobs/graphics/effect-param-numbers.c.h`, compiled as
//! `oracle_*` with the C `lexer.c` and the C library's `strtol`.
//!
//! Compared: fixed tokens (the int edges, leading zeros, signs, C white
//! space, float and hex tokens, tails), every one followed by more digits
//! in the source, then proptests of random texts of up to 25 bytes (digits,
//! signs, spaces, `.`, `x`, `e`, `;`, `]`, `}` and a non-ASCII byte), each
//! with and without the `-` token, and every count at every length up to
//! two past the text, so a length shorter than the digits is reached.
//!
//! Intentional difference (#3464): past `INT_MAX` the C's value depends on
//! the width of `long` (`strtol` saturates at `LONG_MAX`, then the `(int)`
//! cast, implementation-defined there, keeps the low 32 bits where `long`
//! is 64 bits, and saturates before the `-` token is applied where it is
//! 32 bits), and the port saturates to `i32` on every platform. Where the
//! value `strtol` reads or the value after the `-` token (from an
//! independent model) is outside `i32`, the shim is checked against that
//! contract instead of the oracle, which is then not called (it could
//! negate `LONG_MIN`, undefined); whether a count is valid is still
//! compared. Everything else is compared exactly.
//!
//! Left out, as the parser never passes it: a null text (the C passes it
//! to `strtol`; the shims read it as `""` and false, checked here on the
//! Rust side).
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use std::ffi::{CString, c_int};
use std::ptr;

use obs_c_oracle::effect_param_types as c;
use obs_c_oracle::parity;
use obs_graphics::effect_param_types::{array_count, int_value};
use obs_graphics::ffi::effect_param_types as rs;
use proptest::prelude::*;

/// What `strtol(str, NULL, 10)` reads, exactly (C white space, one sign,
/// digits), with the `-` token applied; `None` past 30 digits.
fn exact(text: &[u8], negative: bool) -> Option<i128> {
    let text = text.split(|&b| b == 0).next().unwrap_or_default();
    let spaces = text
        .iter()
        .take_while(|b| matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r'))
        .count();
    let rest = &text[spaces..];
    let (minus, digits) = match rest.split_first() {
        Some((b'-', d)) => (true, d),
        Some((b'+', d)) => (false, d),
        _ => (false, rest),
    };
    let digits: Vec<u8> = digits
        .iter()
        .take_while(|b| b.is_ascii_digit())
        .copied()
        .collect();
    let digits = &digits[digits.iter().take_while(|&&b| b == b'0').count()..];
    if digits.len() > 30 {
        return None;
    }
    let mut n: i128 = 0;
    for &b in digits {
        n = n * 10 + i128::from(b - b'0');
    }
    let n = if minus { -n } else { n };
    Some(if negative { -n } else { n })
}

fn in_int(v: Option<i128>) -> bool {
    v.is_some_and(|v| i32::try_from(v).is_ok())
}

fn saturated(v: Option<i128>) -> i32 {
    match v {
        Some(v) => i32::try_from(v).unwrap_or(if v < 0 { i32::MIN } else { i32::MAX }),
        None => i32::MAX,
    }
}

fn c_string(text: &[u8]) -> CString {
    CString::new(text.split(|&b| b == 0).next().unwrap_or_default()).unwrap()
}

fn shim_int(s: &CString, negative: bool) -> c_int {
    // SAFETY: a valid C string.
    unsafe { rs::obs_effect_param_int_value(s.as_ptr(), negative) }
}

fn oracle_int(s: &CString, negative: bool) -> c_int {
    // SAFETY: a valid C string.
    unsafe { c::oracle_obs_effect_param_int_value(s.as_ptr(), negative) }
}

fn shim_count(s: &CString, len: usize) -> (bool, c_int) {
    let mut count = -12345;
    // SAFETY: a valid C string and count.
    let ok = unsafe { rs::obs_effect_param_array_count(s.as_ptr(), len, &raw mut count) };
    (ok, count)
}

fn oracle_count(s: &CString, len: usize) -> (bool, c_int) {
    let mut count = -12345;
    // SAFETY: a valid C string and count.
    let ok = unsafe { c::oracle_obs_effect_param_array_count(s.as_ptr(), len, &raw mut count) };
    (ok, count)
}

/// One text: the int with and without the `-` token, and the count at
/// every length up to two past the text.
fn check(text: &[u8]) -> Result<(), TestCaseError> {
    let s = c_string(text);
    let bytes = s.as_bytes();
    // strtol()'s own value: the C is exact only while it fits in an int,
    // on every width of long.
    let read = exact(bytes, false);
    for negative in [false, true] {
        let want = exact(bytes, negative);
        let got = shim_int(&s, negative);
        prop_assert_eq!(got, int_value(bytes, negative), "core {:?} {}", s, negative);
        if in_int(read) && in_int(want) {
            prop_assert_eq!(got, oracle_int(&s, negative), "{:?} {}", s, negative);
        } else {
            prop_assert_eq!(got, saturated(want), "fixed {:?} {}", s, negative);
        }
    }
    for len in 0..=bytes.len() + 2 {
        let (ok, count) = shim_count(&s, len);
        let (c_ok, c_count) = oracle_count(&s, len);
        prop_assert_eq!(ok, c_ok, "valid {:?} {}", s, len);
        prop_assert_eq!(
            ok.then_some(count),
            array_count(bytes, len),
            "core {:?} {}",
            s,
            len
        );
        if !ok {
            prop_assert_eq!(count, -12345, "written {:?} {}", s, len);
            continue;
        }
        if in_int(read) {
            prop_assert_eq!(count, c_count, "{:?} {}", s, len);
        } else {
            prop_assert_eq!(count, saturated(read), "fixed {:?} {}", s, len);
        }
    }
    Ok(())
}

const FIXED: &[&str] = &[
    "",
    "0",
    "5",
    "007",
    "123456",
    "1.5",
    "1e3",
    "0x10",
    "12;",
    "4}",
    "4]",
    "12 34",
    "2147483646",
    "2147483647",
    "2147483648",
    "02147483647",
    "3000000000",
    "4294967295",
    "4294967296",
    "9223372036854775807",
    "9223372036854775808",
    "99999999999999999999",
    "+7",
    "-7",
    "-2147483648",
    "-2147483649",
    " 9",
    "\t\n\x0b\x0c\r12",
    "+-7",
    "-",
    "+",
    " ",
    "x",
    "4x",
    "1x2",
    "\u{e9}1",
];

#[test]
fn fixed_tokens() {
    for text in FIXED {
        check(text.as_bytes()).unwrap();
        // The same token followed by more digits in the source.
        check(format!("{text}0123").as_bytes()).unwrap();
    }
}

#[test]
fn long_width_is_what_the_c_was_built_with() {
    // SAFETY: no arguments.
    let long = unsafe { c::oracle_effect_param_numbers_long_size() };
    assert_eq!(long, size_of::<std::ffi::c_long>());
}

#[test]
fn null_text() {
    let mut count = 7;
    // SAFETY: a null text is read as "" and false.
    unsafe {
        assert_eq!(rs::obs_effect_param_int_value(ptr::null(), false), 0);
        assert_eq!(rs::obs_effect_param_int_value(ptr::null(), true), 0);
        assert!(!rs::obs_effect_param_array_count(
            ptr::null(),
            1,
            &raw mut count
        ));
    }
    assert_eq!(count, 7);
}

fn text() -> impl Strategy<Value = Vec<u8>> {
    let byte = prop_oneof![
        8 => b'0'..=b'9',
        1 => Just(b' '),
        1 => Just(b'-'),
        1 => Just(b'+'),
        1 => prop::sample::select(vec![b'.', b'x', b'e', b';', b']', b'}', b'\t', 0xc3]),
    ];
    prop::collection::vec(byte, 0..=25)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(parity::cases(2048)))]

    #[test]
    fn any_text(t in text()) {
        check(&t)?;
    }

    /// Digit runs only, so the edges of i32 and of a 64-bit long come up.
    #[test]
    fn digit_runs(digits in "[0-9]{1,22}", tail in "[;\\]} .x]{0,2}") {
        check(format!("{digits}{tail}").as_bytes())?;
    }
}
