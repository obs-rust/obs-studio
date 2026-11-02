//! Safe core of `libobs/graphics/shader-param-values.c.h`: the number reads
//! of the shader parser (`shader-parser.c`), on a number token's text.
//!
//! - [`array_count`] (`sp_parse_param_array`): the count of `name[count]`;
//! - [`int_value`] and [`float_value`] (`sp_parse_param_assign_intfloat`):
//!   an `int` or `float` default value, negated when a `-` token came first.
//!
//! The count and the `int` are the same reads as the effect parser's
//! (#3464): the `int` is its rule ([`effect_param_types::int_value_by`]),
//! so both parsers read the same `int` the same way, and the count reads
//! the text with its `valid_int_str` and `strtol` steps but refuses a count
//! outside `i32` where the effect parser's saturates.
//!
//! The token's text runs on into the shader source, as `strtol` and
//! `os_strtod` read it. [`array_count_by`], [`int_value_by`] and
//! [`float_value_by`] read it one byte at a time, in the C's order, so the
//! shims read no byte the C would not and never scan the source to its end.
//!
//! `shader-parser.c` keeps every token call, the `CFTOKEN_NUM` tests, the
//! `-` token, the value count of a `floatN` or `floatNxM` default (each of
//! its bytes read in place), the push of each value and every
//! `cf_adderror()`. Nothing here keeps state or calls libobs.
//!
//! Characterized, not endorsed (`test_shader_param_values_steps`), and
//! kept: a float or hex token reads as its leading digits as an `int` or a
//! count (`1.5` is 1, `0x10` is 0); a count checks only the token's length
//! (`valid_int_str`) while `strtol` reads every digit after it; a float
//! past `FLT_MAX` is an infinity, and `os_strtod` reads at most 63 bytes.
//! The float's conversion is passed in: the shim's is libobs's own
//! `os_strtod` (the Rust one in the hybrid libobs, with its documented
//! differences from the C library's), through the C adapter
//! `obs_graphics_os_strtod`, so obs-graphics never reaches obs-util's host
//! code itself (#3468).
//!
//! Two intentional differences (#3468): past `INT_MAX` the C depends on the
//! width of `long` (an `int` of `4294967297` reads as 1 and a count of
//! `3000000000` as -1294967296 with a 64-bit `long`; both saturate before
//! the `-` token with a 32-bit one). [`int_value`] reads the digits
//! exactly, applies the sign and saturates to `i32` on every platform, the
//! effect parser's rule; [`array_count`] refuses a count outside `i32`, so
//! `sp_parse_param_array` drops the parameter as it does for a count that
//! is not a number. Inside `i32` both agree with the C.

use crate::effect_param_types;

/// `os_strtod`'s `char buf[64]`: it copies at most 63 bytes, then a NUL.
const OS_STRTOD_BUF: usize = 64;

/// `shader_param_array_count` (`sp_parse_param_array`): the count of
/// `name[count]`, or `None` when the token's first `len` bytes are not an
/// int (`valid_int_str`, which reads the whole string when `len` is 0).
/// `text` is the number token's text as a C string (it ends at a NUL or at
/// the end of the slice). See [`array_count_by`].
#[must_use]
pub fn array_count(text: &[u8], len: usize) -> Option<i32> {
    array_count_by(|i| text.get(i).copied().unwrap_or(0), len)
}

/// [`array_count`] on the bytes `at(i)` gives, in the C's order:
/// `valid_int_str(str, len)` first, reading what the C loop reads and
/// stopping where it stops, then, only when it passes, what `strtol`
/// reads, each byte once. `at(i)` is asked only once every byte before `i`
/// was read and was not NUL, so the shim can read a C string that runs on
/// into the shader source byte by byte.
///
/// Intentional difference (#3468): `None` for a count outside `i32`. The C
/// casts `strtol`'s `long` to `int`, which wraps with a 64-bit `long` and
/// saturates with a 32-bit one.
#[must_use]
pub fn array_count_by(mut at: impl FnMut(usize) -> u8, len: usize) -> Option<i32> {
    if !effect_param_types::valid_int_by(&mut at, len) {
        return None;
    }
    i32::try_from(effect_param_types::strtol_by(at)).ok()
}

/// `shader_param_int_value` (`sp_parse_param_assign_intfloat`): an `int`
/// default value, `text` read as `strtol(str, NULL, 10)` reads it, to the
/// first byte that is not a digit; `negative` when a `-` token came before
/// it. See [`int_value_by`].
#[must_use]
pub fn int_value(text: &[u8], negative: bool) -> i32 {
    effect_param_types::int_value(text, negative)
}

/// [`int_value`] on the bytes `at(i)` gives, read once each and in order,
/// as `strtol` reads them. The effect parser's `int` value
/// ([`effect_param_types::int_value_by`]), saturated to `i32` (#3468).
#[must_use]
pub fn int_value_by(at: impl FnMut(usize) -> u8, negative: bool) -> i32 {
    effect_param_types::int_value_by(at, negative)
}

/// `shader_param_float_value` (`sp_parse_param_assign_intfloat`): a `float`
/// default value, `os_strtod` of `text` (at most its first 63 bytes) cast to
/// `float`, negated when a `-` token came before it. `strtod` converts the
/// copy as `os_strtod` does. See [`float_value_by`].
#[must_use]
pub fn float_value(text: &[u8], negative: bool, strtod: impl FnOnce(&[u8]) -> f64) -> f32 {
    float_value_by(|i| text.get(i).copied().unwrap_or(0), negative, strtod)
}

/// [`float_value`] on the bytes `at(i)` gives: `os_strtod`'s `strncpy` into
/// its 64-byte buffer reads each byte once, in order, up to the first NUL
/// or 63 bytes. `strtod` gets that copy with the NUL that ends it (at most
/// 64 bytes), so a C conversion can read it as a string.
#[must_use]
pub fn float_value_by(
    mut at: impl FnMut(usize) -> u8,
    negative: bool,
    strtod: impl FnOnce(&[u8]) -> f64,
) -> f32 {
    let mut buf = [0_u8; OS_STRTOD_BUF];
    let mut len = 0_usize;
    for (i, slot) in buf.iter_mut().take(OS_STRTOD_BUF - 1).enumerate() {
        let b = at(i);
        if b == 0 {
            break;
        }
        *slot = b;
        len = i.saturating_add(1);
    }
    let value = narrow(strtod(buf.get(..=len).unwrap_or_default()));
    if negative { -value } else { value }
}

/// `(float)` of a `double`: rounded to nearest, an infinity past `FLT_MAX`.
#[expect(clippy::cast_possible_truncation, reason = "the C's (float) cast")]
fn narrow(value: f64) -> f32 {
    value as f32
}
