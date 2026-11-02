//! Tier 1: the number reads of `libobs/graphics/effect-param-numbers.c.h`,
//! in `obs_graphics::effect_param_types`.
//!
//! `steps_*` mirror `test/cmocka/test_effect_param_numbers_steps.c` one to
//! one (same names, values and expectations) for every value inside `int`
//! range. Past it the C result depends on the width of `long`; `fixed_*`
//! check the intentional difference instead (#3464): the digits are read
//! exactly, the sign applied, and the result saturated to `i32` on every
//! platform. `edge_*` and the proptests cover what the C test leaves out.

use obs_graphics::effect_param_types::{array_count, int_value};
use proptest::prelude::*;

fn check_int(text: &str, positive: i32, negative: i32) {
    assert_eq!(int_value(text.as_bytes(), false), positive, "{text:?}");
    assert_eq!(int_value(text.as_bytes(), true), negative, "-{text:?}");
}

fn check_count(text: &str, len: usize, count: Option<i32>) {
    assert_eq!(array_count(text.as_bytes(), len), count, "{text:?} {len}");
}

#[test]
fn steps_int_values() {
    check_int("0", 0, 0);
    check_int("5", 5, -5);
    check_int("007", 7, -7);
    check_int("123456", 123_456, -123_456);
}

#[test]
fn steps_int_value_reads_to_the_first_non_digit() {
    check_int("1.5", 1, -1);
    check_int("1e3", 1, -1);
    check_int("0x10", 0, 0);
    check_int("12;", 12, -12);
    check_int("4}", 4, -4);
    check_int("12 34", 12, -12);
}

#[test]
fn steps_int_value_edges() {
    check_int("2147483647", i32::MAX, -i32::MAX);
}

#[test]
fn fixed_int_value_edges() {
    // The C gives INT_MIN and INT_MIN with a 64-bit long, INT_MAX and
    // -INT_MAX with a 32-bit one.
    check_int("2147483648", i32::MAX, i32::MIN);
    // C, 64-bit long: -1294967296, 1294967296; 0, 0; -1, 1.
    check_int("3000000000", i32::MAX, i32::MIN);
    check_int("4294967296", i32::MAX, i32::MIN);
    check_int("99999999999999999999", i32::MAX, i32::MIN);
    check_int("000000000000000000002147483647", i32::MAX, -i32::MAX);
}

#[test]
fn steps_array_counts() {
    check_count("0", 1, Some(0));
    check_count("4", 1, Some(4));
    check_count("4]", 1, Some(4));
    check_count("16", 2, Some(16));
}

#[test]
fn steps_array_count_rejects() {
    check_count("1.5", 3, None);
    check_count("x", 1, None);
    check_count("4x", 2, None);
    check_count("", 1, None);
}

#[test]
fn steps_array_count_odd_lengths() {
    check_count("4567", 1, Some(4567));
    check_count("4x", 1, Some(4));
    check_count("12", 0, Some(12));
    check_count("1x", 0, None);
    check_count("-3", 2, Some(-3));
}

#[test]
fn steps_array_count_edges() {
    check_count("2147483647", 10, Some(i32::MAX));
}

#[test]
fn fixed_array_count_edges() {
    // The C gives -1294967296 with a 64-bit long.
    check_count("3000000000", 10, Some(i32::MAX));
    check_count("99999999999999999999", 20, Some(i32::MAX));
    check_count("-3000000000", 11, Some(i32::MIN));
}

#[test]
fn edge_int_value_text_strtol_skips() {
    // strtol() skips C white space and takes one sign of its own; the
    // parser never passes either (a number token starts with a digit).
    check_int(" \t\n\x0b\x0c\r9", 9, -9);
    check_int("+7", 7, -7);
    check_int("-7", -7, 7);
    check_int("-2147483648", i32::MIN, i32::MAX);
    check_int("+-7", 0, 0);
    check_int("", 0, 0);
    check_int("-", 0, 0);
    check_int(" ", 0, 0);
}

#[test]
fn edge_text_ends_at_nul() {
    assert_eq!(int_value(b"12\x0034", false), 12);
    assert_eq!(array_count(b"12\x0034", 0), Some(12));
    assert_eq!(array_count(b"12\x0034", 4), Some(12));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(512)))]

    /// Every int prints and reads back as itself, with the '-' as a token
    /// before it or as the string's own sign.
    #[test]
    fn int_value_round_trips(v in any::<i32>(), tail in "[;,}\\] .x]{0,3}") {
        let magnitude = i64::from(v).unsigned_abs();
        prop_assert_eq!(int_value(format!("{magnitude}{tail}").as_bytes(), v < 0), v);
        prop_assert_eq!(int_value(format!("{v}{tail}").as_bytes(), false), v);
    }

    /// Any digit string saturates to i32 after the sign.
    #[test]
    fn int_value_saturates(digits in "[0-9]{1,30}", negative in any::<bool>()) {
        let exact: i128 = digits.trim_start_matches('0').chars().take(30).fold(0i128, |n, c| {
            n * 10 + i128::from(c.to_digit(10).unwrap_or(0))
        });
        let signed = if negative { -exact } else { exact };
        let want = signed.clamp(i128::from(i32::MIN), i128::from(i32::MAX));
        prop_assert_eq!(i128::from(int_value(digits.as_bytes(), negative)), want);
    }

    /// A count is the int of the text when its first len bytes are digits.
    #[test]
    fn array_count_reads_like_int_value(digits in "[0-9]{1,12}", tail in "[x.\\]]{0,2}", cut in 0usize..16) {
        let text = format!("{digits}{tail}");
        let len = cut.min(text.len());
        let valid = if len == 0 { tail.is_empty() } else { len <= digits.len() };
        let want = valid.then(|| int_value(text.as_bytes(), false));
        prop_assert_eq!(array_count(text.as_bytes(), len), want);
    }
}
