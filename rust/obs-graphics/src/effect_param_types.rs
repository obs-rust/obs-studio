//! Safe core of `libobs/graphics/effect-param-types.c.h`: the parameter
//! type decisions of the effect parser (`effect-parser.c`), on a type name;
//! and of `libobs/graphics/effect-param-numbers.c.h`: its number reads.
//!
//! - [`param_type`] (`get_effect_param_type`, for `ep_compile_annotations`
//!   and `ep_compile_param`): the shader parameter type of a type name. It
//!   is the map of `get_shader_param_type`, so it calls
//!   [`shader_param_type`](crate::shader_param_names::shader_param_type).
//! - [`route`] (`ep_parse_param_assignment_val`): the default-value parser
//!   a parameter gets.
//! - `ep_parse_param_assign_intfloat_array`, on the dimensions after the
//!   `int` or `float` prefix, one function per read, each asked where the C
//!   read its byte: [`rows_bad`] (`effect-parser.c` then adds the row error),
//!   [`rows`], the value count so far, [`has_cols`], then [`cols_bad`] (it
//!   adds the column error) and [`cols`], the count times the column digit.
//! - [`int_value`] (`ep_parse_param_assign_intfloat`): an `int` value, and
//!   [`array_count`] (`ep_parse_param_array`): the count of `name[count]`,
//!   on the number token's text, which runs on into the effect source as
//!   `strtol` reads it. [`int_value_by`] and [`array_count_by`] read it one
//!   byte at a time, in one pass and in the C's order, so the shims read no
//!   byte the C would not.
//!
//! Names compare as bytes, case-sensitively, as `strcmp` and `astrcmp_n`
//! do. `effect-parser.c` keeps every token call, every `cf_adderror()`, the
//! token type tests, the float values, the push of each value, the brace
//! and constructor handling, the loop over the values and the compile.
//! Nothing here keeps state or calls libobs.
//!
//! Characterized, not endorsed (`test_effect_param_types_steps`), and kept:
//! any name starting with `texture` is a texture; `strings` takes the string
//! parser and `int1` or `float3x3` array values, with an unknown type; a bad
//! row or column digit still counts, as a C `char` minus `'0'` (`floatx` is
//! 72 values, the terminator after an `x` is `-48`). The digit is a
//! [`c_char`], so a non-ASCII byte counts as the C does on each target:
//! negative where `char` is signed (x86-64, MSVC), not on arm64 Linux.
//!
//! Characterized, not endorsed (`test_effect_param_numbers_steps`), and
//! kept: a float or hex token reads as its leading digits (`1.5` is 1,
//! `0x10` is 0), and a count checks only the token's length
//! (`valid_int_str`) while `strtol` reads every digit after it.
//!
//! One intentional difference (#3464), in the number reads: past `INT_MAX`
//! the C depends on the width of `long` (`3000000000` reads as -1294967296
//! with a 64-bit `long`, `-2147483648` as -2147483647 with a 32-bit one);
//! [`int_value`] and [`array_count`] read the digits exactly, apply the
//! sign and saturate to `i32` on every platform. The type decisions have no
//! intentional difference.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

use core::ffi::c_char;

use crate::shader_param_names::shader_param_type;

/// The default-value parser of a parameter (`enum obs_effect_param_route`),
/// in the order `ep_parse_param_assignment_val()` tests them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum Route {
    /// `ep_parse_param_assign_texture`: the texture flag.
    Texture = 0,
    /// `ep_parse_param_assign_intfloat(.., false)`: exactly `int`.
    Int = 1,
    /// `ep_parse_param_assign_intfloat(.., true)`: exactly `float`.
    Float = 2,
    /// `ep_parse_param_assign_intfloat_array(.., false)`: an `int` prefix.
    IntArray = 3,
    /// `ep_parse_param_assign_intfloat_array(.., true)`: a `float` prefix.
    FloatArray = 4,
    /// `ep_parse_param_assign_string`: a `string` prefix.
    String = 5,
    /// `ep_parse_param_assign_bool`: exactly `bool`.
    Bool = 6,
    /// None: the "Invalid type" error.
    Invalid = 7,
}

/// `obs_effect_param_type()`: the `GS_SHADER_PARAM_*` type of `name`.
#[must_use]
pub fn param_type(name: &[u8]) -> i32 {
    shader_param_type(name)
}

/// `obs_effect_param_route()`: the parser of a parameter of type `name`, a
/// texture when `is_texture`.
#[must_use]
pub fn route(name: &[u8], is_texture: bool) -> Route {
    match name {
        _ if is_texture => Route::Texture,
        b"int" => Route::Int,
        b"float" => Route::Float,
        // astrcmp_n(type, prefix, len): any name that starts with it.
        n if n.starts_with(b"int") => Route::IntArray,
        n if n.starts_with(b"float") => Route::FloatArray,
        n if n.starts_with(b"string") => Route::String,
        b"bool" => Route::Bool,
        _ => Route::Invalid,
    }
}

/// The bytes after the `float` (`is_float`) or `int` prefix of `name`.
fn dims(name: &[u8], is_float: bool) -> &[u8] {
    name.get(if is_float { 5 } else { 3 }..).unwrap_or_default()
}

/// Byte `i` of the dimensions; past the end of `name` reads as the
/// terminator, `0`.
fn dim(name: &[u8], is_float: bool, i: usize) -> u8 {
    dims(name, is_float).get(i).copied().unwrap_or(0)
}

/// `obs_effect_param_rows_bad()`: the row digit is outside 1-4.
#[must_use]
pub fn rows_bad(name: &[u8], is_float: bool) -> bool {
    !is_dim(dim(name, is_float, 0))
}

/// `obs_effect_param_rows()`: the row digit minus `'0'`, the value count
/// before any column.
#[must_use]
pub fn rows(name: &[u8], is_float: bool) -> i32 {
    digit(dim(name, is_float, 0))
}

/// `obs_effect_param_has_cols()`: the row digit is followed by an `x`.
#[must_use]
pub fn has_cols(name: &[u8], is_float: bool) -> bool {
    dim(name, is_float, 1) == b'x'
}

/// `obs_effect_param_cols_bad()`: the column digit after the `x` is outside
/// 1-4.
#[must_use]
pub fn cols_bad(name: &[u8], is_float: bool) -> bool {
    !is_dim(dim(name, is_float, 2))
}

/// `obs_effect_param_cols()`: `count` times the column digit minus `'0'`.
/// It wraps where the C `int` would overflow; from [`rows`] it never does
/// (each digit is within -176..=207).
#[must_use]
pub fn cols(name: &[u8], is_float: bool, count: i32) -> i32 {
    count.wrapping_mul(digit(dim(name, is_float, 2)))
}

/// A row or column digit, `'1'` to `'4'`.
fn is_dim(b: u8) -> bool {
    (b'1'..=b'4').contains(&b)
}

/// `b - '0'` with `b` a C `char`: signed or not as on the target.
fn digit(b: u8) -> i32 {
    i32::from(b as c_char).wrapping_sub(i32::from(b'0'))
}

/// `obs_effect_param_int_value` (`ep_parse_param_assign_intfloat`): an `int`
/// default value. `text` is the number token's text as a C string (it ends
/// at a NUL or at the end of the slice), read on past the token as
/// `strtol(str, NULL, 10)` reads it, to the first byte that is not a digit;
/// `negative` when a `-` token came before it. See [`int_value_by`].
#[must_use]
pub fn int_value(text: &[u8], negative: bool) -> i32 {
    int_value_by(|i| byte(text, i), negative)
}

/// [`int_value`] on the bytes `at(i)` gives, read once each and in order,
/// as `strtol` reads them: C white space, one sign, the digits and the byte
/// that stops them, nothing after it. `at(i)` is asked only once every byte
/// before `i` was read and was not NUL, so the shim can read a C string
/// that runs on into the effect source byte by byte.
///
/// Intentional difference (#3464): the digits are read exactly, the sign
/// applied, and the value saturated to `i32`. The C saturates at `LONG_MAX`
/// first and then casts to `int`, so past `INT_MAX` it gives another value
/// on each width of `long`; inside `int` range the two agree.
#[must_use]
pub fn int_value_by(at: impl FnMut(usize) -> u8, negative: bool) -> i32 {
    let value = strtol_by(at);
    saturate(if negative {
        value.saturating_neg()
    } else {
        value
    })
}

/// `obs_effect_param_array_count` (`ep_parse_param_array`): the count of
/// `name[count]`, or `None` when the token's first `len` bytes are not an
/// int (`valid_int_str`, which reads the whole string when `len` is 0).
/// `text` is as for [`int_value`]. See [`array_count_by`].
#[must_use]
pub fn array_count(text: &[u8], len: usize) -> Option<i32> {
    array_count_by(|i| byte(text, i), len)
}

/// [`array_count`] on the bytes `at(i)` gives, as for [`int_value_by`]:
/// `valid_int_str(str, len)` first, reading what the C loop reads and
/// stopping where it stops, then, only when it passes, what `strtol` reads.
/// The count is saturated to `i32` as in [`int_value_by`] (#3464).
#[must_use]
pub fn array_count_by(mut at: impl FnMut(usize) -> u8, len: usize) -> Option<i32> {
    valid_int_by(&mut at, len).then(|| saturate(strtol_by(at)))
}

/// `valid_int_str(str, n)` of util/lexer.c, reading `at` in its order:
/// the first byte, the whole string when `n` is 0 (its `strlen`), a sign,
/// then each digit and the byte after it until a byte is not a digit, the
/// string ends or `n` digits were checked. The shader parser's count
/// (`shader_param_values`) reads with it too.
pub(crate) fn valid_int_by(at: &mut impl FnMut(usize) -> u8, n: usize) -> bool {
    if at(0) == 0 {
        return false;
    }
    let mut n = if n == 0 {
        let mut len = 1_usize;
        while at(len) != 0 {
            len = len.saturating_add(1);
        }
        len
    } else {
        n
    };
    let mut i = usize::from(matches!(at(0), b'-' | b'+'));
    loop {
        if !at(i).is_ascii_digit() {
            return false;
        }
        i = i.saturating_add(1);
        // `*++str && --n`
        if at(i) == 0 {
            return true;
        }
        n = n.saturating_sub(1);
        if n == 0 {
            return true;
        }
    }
}

/// What `strtol(str, NULL, 10)` reads, exactly, in `i64`: 0 without digits.
/// Past `i32` the magnitude stops growing, since the callers only saturate
/// or refuse it (the shader parser's count). One pass: each byte is asked
/// once, the stopping byte last.
pub(crate) fn strtol_by(mut at: impl FnMut(usize) -> u8) -> i64 {
    let mut i = 0_usize;
    let mut b = at(i);
    while is_c_space(b) {
        i = i.saturating_add(1);
        b = at(i);
    }
    let minus = b == b'-';
    if matches!(b, b'-' | b'+') {
        i = i.saturating_add(1);
        b = at(i);
    }
    let cap = i64::from(i32::MAX).saturating_add(2);
    let mut magnitude = 0_i64;
    while b.is_ascii_digit() {
        magnitude = magnitude
            .saturating_mul(10)
            .saturating_add(i64::from(b.wrapping_sub(b'0')))
            .min(cap);
        i = i.saturating_add(1);
        b = at(i);
    }
    if minus {
        magnitude.saturating_neg()
    } else {
        magnitude
    }
}

fn saturate(value: i64) -> i32 {
    i32::try_from(value).unwrap_or(if value < 0 { i32::MIN } else { i32::MAX })
}

/// Byte `i` of `text`, 0 past its end: a slice reads as a C string.
fn byte(text: &[u8], i: usize) -> u8 {
    text.get(i).copied().unwrap_or(0)
}

/// `isspace` in the "C" locale.
fn is_c_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}
