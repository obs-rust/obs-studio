//! Tier 1: the number reads of `libobs/graphics/shader-param-values.c.h`,
//! in `obs_graphics::shader_param_values`.
//!
//! `steps_*` mirror `test/cmocka/test_shader_param_values_steps.c` one to
//! one (same names, values and expectations) for every value inside `int`
//! range. Past it the C result depends on the width of `long`; `fixed_*`
//! check the two intentional differences instead (#3468), on every
//! platform: an `int` value is read exactly, the sign applied, and the
//! result saturated to `i32`, by the effect parser's rule (#3464), so both
//! parsers read the same `int` the same way (`same_*`); and a count outside
//! `i32` is refused, as a count that is not a number is. `edge_*` and the
//! proptests cover what the C test leaves out, with `reads_*`: which bytes
//! of the text are read, in what order, as the C's `valid_int_str()`,
//! `strtol()` and `os_strtod()` read them.

use obs_graphics::effect_param_types;
use obs_graphics::shader_param_values::{
    array_count, array_count_by, float_value, float_value_by, int_value, int_value_by,
};
use proptest::prelude::*;

fn check_count(text: &str, len: usize, count: Option<i32>) {
    assert_eq!(array_count(text.as_bytes(), len), count, "{text:?} {len}");
}

fn check_int(text: &str, positive: i32, negative: i32) {
    assert_eq!(int_value(text.as_bytes(), false), positive, "{text:?}");
    assert_eq!(int_value(text.as_bytes(), true), negative, "-{text:?}");
}

/// The value and its negation, bit for bit (a -0 keeps its sign).
fn check_float(text: &[u8], positive: f32) {
    let got = float_value(text, false);
    let negative = float_value(text, true);
    assert_eq!(got.to_bits(), positive.to_bits(), "{text:?}: {got}");
    assert_eq!(
        negative.to_bits(),
        (-positive).to_bits(),
        "-{text:?}: {negative}"
    );
}

#[test]
fn steps_array_counts() {
    check_count("0", 1, Some(0));
    check_count("7", 1, Some(7));
    check_count("007", 3, Some(7));
    check_count("16", 2, Some(16));
    check_count("4]", 1, Some(4));
    check_count("+7", 2, Some(7));
    check_count("-3", 2, Some(-3));
}

#[test]
fn steps_array_count_rejects() {
    check_count("1.5", 3, None);
    check_count("1e3", 3, None);
    check_count("x", 1, None);
    check_count("4x", 2, None);
    check_count("+", 1, None);
    check_count("", 1, None);
}

#[test]
fn steps_array_count_odd_lengths() {
    check_count("4567", 1, Some(4567));
    check_count("4x", 1, Some(4));
    check_count("12", 0, Some(12));
    check_count("1x", 0, None);
}

#[test]
fn steps_array_count_edges() {
    check_count("2147483647", 10, Some(i32::MAX));
}

#[test]
fn fixed_array_count_edges() {
    // C, 64-bit long: INT_MIN, 1, -1 and -1294967296; 32-bit long:
    // INT_MAX each time.
    check_count("2147483648", 10, None);
    check_count("4294967297", 10, None);
    check_count("99999999999999999999", 20, None);
    check_count("3000000000", 10, None);
    // The text's own sign (the parser never passes one).
    check_count("-2147483648", 11, Some(i32::MIN));
    check_count("-2147483649", 11, None);
    check_count("+2147483648", 11, None);
    check_count("000000000000000000002147483647", 30, Some(i32::MAX));
}

#[test]
fn steps_int_values() {
    check_int("0", 0, 0);
    check_int("7", 7, -7);
    check_int("+7", 7, -7);
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
    check_int("x", 0, 0);
}

#[test]
fn steps_int_value_edges() {
    check_int("2147483647", i32::MAX, -i32::MAX);
}

#[test]
fn fixed_int_value_edges() {
    // C, 64-bit long: INT_MIN and INT_MIN; 32-bit long: INT_MAX and
    // -INT_MAX.
    check_int("2147483648", i32::MAX, i32::MIN);
    // C, 64-bit long: 1, -1; and -1, 1.
    check_int("4294967297", i32::MAX, i32::MIN);
    check_int("99999999999999999999", i32::MAX, i32::MIN);
    check_int("000000000000000000002147483647", i32::MAX, -i32::MAX);
}

#[test]
fn steps_float_values() {
    check_float(b"1.5", 1.5);
    check_float(b"1e3", 1000.0);
    check_float(b".5", 0.5);
    // The C's (float)0.1, the same bits as 0.1_f32.
    check_float(b"0.1", 0.1_f32);
    check_float(b"7", 7.0);
    check_float(b"2.5}", 2.5);
    check_float(b"0x10", 16.0);
    check_float(b"0", 0.0);
    check_float(b"x", 0.0);
}

#[test]
fn steps_float_value_edges() {
    check_float(b"3.4e39", f32::INFINITY);
    check_float(b"1e-50", 0.0);
}

#[test]
fn steps_float_value_reads_63_bytes() {
    let mut text = [b'0'; 64];
    text[..2].copy_from_slice(b"1.");
    text[61..63].copy_from_slice(b"e1");
    check_float(&text[..63], 10.0);
    text[61..64].copy_from_slice(b"0e1");
    check_float(&text, 1.0);
}

#[test]
fn edge_strtol_skips() {
    // strtol() and strtod() skip C white space and take one sign of their
    // own; the parser never passes either (a number token starts with a
    // digit or a '.').
    check_int(" \t\n\x0b\x0c\r9", 9, -9);
    check_int("-7", -7, 7);
    check_int("-2147483648", i32::MIN, i32::MAX);
    check_int("+-7", 0, 0);
    check_int("", 0, 0);
    check_int("-", 0, 0);
    check_float(b" \t\n\x0b\x0c\r9", 9.0);
    check_float(b"-7", -7.0);
    check_float(b"", 0.0);
}

#[test]
fn edge_text_ends_at_nul() {
    assert_eq!(int_value(b"12\x0034", false), 12);
    assert_eq!(array_count(b"12\x0034", 0), Some(12));
    assert_eq!(array_count(b"12\x0034", 4), Some(12));
    check_float(b"1.5\x009", 1.5);
}

/// The indexes `at` is asked, in order, reading `text` as a C string.
fn reads(text: &[u8], run: impl FnOnce(&mut dyn FnMut(usize) -> u8)) -> Vec<usize> {
    let mut asked = Vec::new();
    run(&mut |i| {
        asked.push(i);
        text.get(i).copied().unwrap_or(0)
    });
    asked
}

fn int_reads(text: &str) -> Vec<usize> {
    reads(text.as_bytes(), |at| {
        let _ = int_value_by(at, false);
    })
}

fn count_reads(text: &str, len: usize) -> Vec<usize> {
    reads(text.as_bytes(), |at| {
        let _ = array_count_by(at, len);
    })
}

fn float_reads(text: &str) -> Vec<usize> {
    reads(text.as_bytes(), |at| {
        let _ = float_value_by(at, false);
    })
}

#[test]
fn reads_int_value_once_each_to_the_stop() {
    // strtol(): white space, a sign, the digits and the byte that stops
    // them, each once.
    assert_eq!(int_reads("12;x99"), [0, 1, 2]);
    assert_eq!(int_reads("  5)7"), [0, 1, 2, 3]);
    assert_eq!(int_reads("+7;"), [0, 1, 2]);
    assert_eq!(int_reads("7"), [0, 1]);
    assert_eq!(int_reads("x1"), [0]);
}

#[test]
fn reads_array_count_where_valid_int_str_stops() {
    // valid_int_str("1.5];...", 3) stops at the '.': strtol() never runs.
    assert_eq!(count_reads("1.5];uniform", 3), [0, 0, 0, 1, 1]);
    // "4];..." at length 1: the check reads the digit and the byte after
    // it, then strtol() reads the same two.
    assert_eq!(count_reads("4];uniform", 1), [0, 0, 0, 1, 0, 1]);
    // A zero length reads the whole string first (its strlen).
    assert_eq!(count_reads("12", 0), [0, 1, 2, 0, 0, 1, 1, 2, 0, 1, 2]);
    assert_eq!(count_reads("", 1), [0]);
}

#[test]
fn reads_float_value_to_the_nul_or_63_bytes() {
    // os_strtod()'s strncpy(): each byte once, to the NUL or 63 bytes.
    assert_eq!(float_reads("1.5"), [0, 1, 2, 3]);
    assert_eq!(float_reads(""), [0]);
    let long = "1".repeat(70);
    assert_eq!(float_reads(&long), (0..63).collect::<Vec<_>>());
    let exact = "2".repeat(63);
    assert_eq!(float_reads(&exact), (0..63).collect::<Vec<_>>());
    let short = "3".repeat(62);
    assert_eq!(float_reads(&short), (0..63).collect::<Vec<_>>());
}

#[test]
fn edge_long_leading_zeros_read_in_one_pass() {
    let text = format!("{}7;", "0".repeat(1_000_000));
    assert_eq!(int_reads(&text).len(), 1_000_002);
    assert_eq!(int_value(text.as_bytes(), true), -7);
    assert_eq!(array_count(text.as_bytes(), 1_000_001), Some(7));
    // os_strtod() stops at 63 bytes, all zeros here.
    assert_eq!(float_value(text.as_bytes(), false).to_bits(), 0);
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
        let exact: i128 = digits.chars().fold(0i128, |n, c| {
            n * 10 + i128::from(c.to_digit(10).unwrap_or(0))
        });
        let signed = if negative { -exact } else { exact };
        let want = signed.clamp(i128::from(i32::MIN), i128::from(i32::MAX));
        prop_assert_eq!(i128::from(int_value(digits.as_bytes(), negative)), want);
    }

    /// A count is the int of the text when its first len bytes are digits
    /// and it fits an int, and refused otherwise.
    #[test]
    fn array_count_reads_like_int_value(digits in "[0-9]{1,12}", tail in "[x.\\]]{0,2}", cut in 0usize..16) {
        let text = format!("{digits}{tail}");
        let len = cut.min(text.len());
        let valid = if len == 0 { tail.is_empty() } else { len <= digits.len() };
        let exact: i128 = digits.chars().fold(0i128, |n, c| {
            n * 10 + i128::from(c.to_digit(10).unwrap_or(0))
        });
        let want = (valid && exact <= i128::from(i32::MAX)).then(|| int_value(text.as_bytes(), false));
        prop_assert_eq!(array_count(text.as_bytes(), len), want);
    }

    /// The shader parser reads an int as the effect parser does, and a
    /// count too, except that it refuses one the effect parser saturates.
    #[test]
    fn same_as_the_effect_parser(text in "[0-9+ .xe;\\]-]{0,24}", negative in any::<bool>(), len in 0usize..26) {
        let bytes = text.as_bytes();
        prop_assert_eq!(int_value(bytes, negative), effect_param_types::int_value(bytes, negative));
        let effect = effect_param_types::array_count(bytes, len);
        match array_count(bytes, len) {
            Some(count) => prop_assert_eq!(effect, Some(count)),
            None => prop_assert!(matches!(effect, None | Some(i32::MIN | i32::MAX)), "{:?}", effect),
        }
    }

    /// Every finite float printed as its double (shortest, with an exponent,
    /// well inside 63 bytes) reads back as itself, and
    /// the '-' token flips its sign bit.
    #[test]
    fn float_value_round_trips(bits in any::<u32>(), tail in "[;,} ]{0,2}") {
        let v = f32::from_bits(bits);
        prop_assume!(v.is_finite());
        let text = format!("{:e}{tail}", f64::from(v.abs()));
        prop_assert_eq!(float_value(text.as_bytes(), false).to_bits(), v.abs().to_bits());
        prop_assert_eq!(float_value(text.as_bytes(), true).to_bits(), (-v.abs()).to_bits());
    }

    /// Past 63 bytes nothing is read.
    #[test]
    fn float_value_reads_63_bytes(text in "[0-9.e+]{64,80}", negative in any::<bool>()) {
        let bytes = text.as_bytes();
        prop_assert_eq!(
            float_value(bytes, negative).to_bits(),
            float_value(&bytes[..63], negative).to_bits()
        );
    }
}
