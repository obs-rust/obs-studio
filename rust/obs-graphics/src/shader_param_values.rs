//! Safe core of `libobs/graphics/shader-param-values.c.h`: the number reads
//! of the shader parser (`shader-parser.c`), on a number token's text.
//!
//! - [`array_count`] (`sp_parse_param_array`): the count of `name[count]`;
//! - [`int_value`] and [`float_value`] (`sp_parse_param_assign_intfloat`):
//!   an `int` or `float` default value, negated when a `-` token came first.
//!
//! The token's text runs on into the shader source, as `strtol` and
//! `os_strtod` read it. [`array_count_by`], [`int_value_by`] and
//! [`float_value_by`] read it one byte at a time, in the C's order, so the
//! shims read no byte the C would not.
//!
//! `shader-parser.c` keeps every token call, the `CFTOKEN_NUM` tests, the
//! `-` token, the push of each value and every `cf_adderror()`. The value
//! count of a `floatN` or `floatNxM` default is the effect parser's type
//! rules ([`crate::effect_param_types`]). Nothing here keeps state or calls
//! libobs.

/// `shader_param_array_count` (`sp_parse_param_array`): the count of
/// `name[count]`, or `None` when the token's first `len` bytes are not an
/// int (`valid_int_str`, which reads the whole string when `len` is 0).
/// `text` is the number token's text as a C string (it ends at a NUL or at
/// the end of the slice). See [`array_count_by`].
#[expect(clippy::todo, reason = "RED: the port follows")]
#[must_use]
pub fn array_count(text: &[u8], len: usize) -> Option<i32> {
    let _ = (text, len);
    todo!()
}

/// [`array_count`] on the bytes `at(i)` gives.
#[expect(clippy::todo, reason = "RED: the port follows")]
#[must_use]
pub fn array_count_by(at: impl FnMut(usize) -> u8, len: usize) -> Option<i32> {
    let _ = (at, len);
    todo!()
}

/// `shader_param_int_value` (`sp_parse_param_assign_intfloat`): an `int`
/// default value, `text` read as `strtol(str, NULL, 10)` reads it, to the
/// first byte that is not a digit; `negative` when a `-` token came before
/// it. See [`int_value_by`].
#[expect(clippy::todo, reason = "RED: the port follows")]
#[must_use]
pub fn int_value(text: &[u8], negative: bool) -> i32 {
    let _ = (text, negative);
    todo!()
}

/// [`int_value`] on the bytes `at(i)` gives.
#[expect(clippy::todo, reason = "RED: the port follows")]
#[must_use]
pub fn int_value_by(at: impl FnMut(usize) -> u8, negative: bool) -> i32 {
    let _ = (at, negative);
    todo!()
}

/// `shader_param_float_value` (`sp_parse_param_assign_intfloat`): a `float`
/// default value, `os_strtod` of `text` (at most its first 63 bytes) cast to
/// `float`, negated when a `-` token came before it. See
/// [`float_value_by`].
#[expect(clippy::todo, reason = "RED: the port follows")]
#[must_use]
pub fn float_value(text: &[u8], negative: bool) -> f32 {
    let _ = (text, negative);
    todo!()
}

/// [`float_value`] on the bytes `at(i)` gives.
#[expect(clippy::todo, reason = "RED: the port follows")]
#[must_use]
pub fn float_value_by(at: impl FnMut(usize) -> u8, negative: bool) -> f32 {
    let _ = (at, negative);
    todo!()
}
