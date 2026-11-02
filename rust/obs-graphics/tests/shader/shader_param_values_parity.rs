//! Tier 3: the Rust shims and the core read shader numbers exactly like the
//! original `libobs/graphics/shader-param-values.c.h`, compiled as
//! `oracle_*` with the C `lexer.c`, the C `platform-number.c` and the C
//! library's `strtol` and `strtod`.
//!
//! Compared: fixed tokens (the int edges, leading zeros, signs, C white
//! space, float and hex tokens, exponents, tails, texts around `os_strtod`'s
//! 63 bytes), every one followed by more digits in the source, then
//! proptests of random texts of up to 25 bytes (digits, signs, spaces, `.`,
//! `x`, `e`, `;`, `]`, `}` and a non-ASCII byte), each with and without the
//! `-` token, every count at every length up to two past the text, so a
//! length shorter than the digits is reached, digit runs past the edges of
//! `i32` and of a 64-bit `long`, and float texts with an exponent.
//!
//! Two intentional differences (#3468): past `INT_MAX` the C's value
//! depends on the width of `long` (`strtol` saturates at `LONG_MAX`, then
//! the `(int)` cast, implementation-defined there, keeps the low 32 bits
//! where `long` is 64 bits, and saturates before the `-` token is applied
//! where it is 32 bits). The port saturates an `int` value to `i32`, the
//! effect parser's rule (#3464), and refuses a count outside `i32`, on
//! every platform. Where the value `strtol` reads or the value after the
//! `-` token (from an independent model) is outside `i32`, the shim is
//! checked against that contract instead of the oracle, which is then not
//! called for the int (it could negate `LONG_MIN`, undefined). Everything
//! else, the float bits included, is compared exactly.
//!
//! The value count of a `floatNxM` default stays C in `shader-parser.c`.
//!
//! Left out, as the parser never passes it: a null text (the C passes it
//! to `strtol` or `os_strtod`; the shims read it as `""` and false, checked
//! here on the Rust side).
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use std::ffi::{CString, c_int};
use std::ptr;

use obs_c_oracle::parity;
use obs_c_oracle::shader_param_values as c;
use obs_graphics::ffi::shader_param_values as rs;
use obs_graphics::shader_param_values::{array_count, float_value, int_value};
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
    unsafe { rs::shader_param_int_value(s.as_ptr(), negative) }
}

fn oracle_int(s: &CString, negative: bool) -> c_int {
    // SAFETY: a valid C string.
    unsafe { c::oracle_shader_param_int_value(s.as_ptr(), negative) }
}

fn shim_count(s: &CString, len: usize) -> (bool, c_int) {
    let mut count = -12345;
    // SAFETY: a valid C string and count.
    let ok = unsafe { rs::shader_param_array_count(s.as_ptr(), len, &raw mut count) };
    (ok, count)
}

fn oracle_count(s: &CString, len: usize) -> (bool, c_int) {
    let mut count = -12345;
    // SAFETY: a valid C string and count.
    let ok = unsafe { c::oracle_shader_param_array_count(s.as_ptr(), len, &raw mut count) };
    (ok, count)
}

fn shim_float(s: &CString, negative: bool) -> u32 {
    // SAFETY: a valid C string.
    unsafe { rs::shader_param_float_value(s.as_ptr(), negative) }.to_bits()
}

fn oracle_float(s: &CString, negative: bool) -> u32 {
    // SAFETY: a valid C string.
    unsafe { c::oracle_shader_param_float_value(s.as_ptr(), negative) }.to_bits()
}

/// One text: the int and the float with and without the `-` token, and the
/// count at every length up to two past the text.
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

        let got = shim_float(&s, negative);
        prop_assert_eq!(
            got,
            float_value(bytes, negative).to_bits(),
            "core float {:?} {}",
            s,
            negative
        );
        prop_assert_eq!(
            got,
            oracle_float(&s, negative),
            "float {:?} {}",
            s,
            negative
        );
    }
    for len in 0..=bytes.len() + 2 {
        let (ok, count) = shim_count(&s, len);
        let (c_ok, c_count) = oracle_count(&s, len);
        prop_assert_eq!(
            ok.then_some(count),
            array_count(bytes, len),
            "core {:?} {}",
            s,
            len
        );
        if !ok {
            prop_assert_eq!(count, -12345, "written {:?} {}", s, len);
        }
        if in_int(read) {
            prop_assert_eq!((ok, count), (c_ok, c_count), "{:?} {}", s, len);
        } else {
            // A count outside int is refused, valid text or not.
            prop_assert!(!ok, "fixed {:?} {}", s, len);
        }
    }
    Ok(())
}

const FIXED: &[&str] = &[
    "",
    "0",
    "7",
    "+7",
    "007",
    "123456",
    "1.5",
    "1e3",
    ".5",
    "0.1",
    "3.4e39",
    "1e-50",
    "1e",
    "1e+",
    "2.5e-3",
    "0x10",
    "0x1p3",
    "12;",
    "4}",
    "4]",
    "2.5}",
    "12 34",
    "2147483646",
    "2147483647",
    "2147483648",
    "02147483647",
    "3000000000",
    "4294967295",
    "4294967296",
    "4294967297",
    "9223372036854775807",
    "9223372036854775808",
    "99999999999999999999",
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
    // Around os_strtod()'s 63 bytes: the '1' of "e1" as byte 63 and 64.
    "1.00000000000000000000000000000000000000000000000000000000000e1",
    "1.000000000000000000000000000000000000000000000000000000000000e1",
    "000000000000000000000000000000000000000000000000000000000000002.5",
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
    let long = unsafe { c::oracle_shader_param_values_long_size() };
    assert_eq!(long, size_of::<std::ffi::c_long>());
}

#[test]
fn null_text() {
    let mut count = 7;
    // SAFETY: a null text is read as "" and false.
    unsafe {
        assert_eq!(rs::shader_param_int_value(ptr::null(), false), 0);
        assert_eq!(rs::shader_param_int_value(ptr::null(), true), 0);
        assert_eq!(
            rs::shader_param_float_value(ptr::null(), false).to_bits(),
            0
        );
        assert_eq!(
            rs::shader_param_float_value(ptr::null(), true).to_bits(),
            (-0.0_f32).to_bits()
        );
        assert!(!rs::shader_param_array_count(
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

    /// Float texts as a shader writes them, with an exponent, some long
    /// enough to reach os_strtod()'s 63 bytes.
    #[test]
    fn float_texts(
        zeros in 0usize..64,
        int in "[0-9]{0,8}",
        frac in "[0-9]{0,12}",
        exp in "(e[+-]?[0-9]{1,3})?",
        tail in "[;,} ]{0,2}",
    ) {
        check(format!("{}{int}.{frac}{exp}{tail}", "0".repeat(zeros)).as_bytes())?;
    }
}
