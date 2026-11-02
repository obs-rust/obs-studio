//! C ABI for `libobs/graphics/effect-param-types.c.h`, the parameter type
//! decisions of the effect parser, and `effect-param-numbers.c.h`, its
//! number reads, declared in `libobs/graphics/effect-param-types.h` without
//! `EXPORT`.
//!
//! `effect-parser.c` (still C) calls these where it made each decision:
//! `ep_compile_annotations()` and `ep_compile_param()` ask
//! [`obs_effect_param_type`], `ep_parse_param_assignment_val()` switches on
//! [`obs_effect_param_route`], and `ep_parse_param_assign_intfloat_array()`
//! asks [`obs_effect_param_rows_bad`] (then adds the row error),
//! [`obs_effect_param_rows`], [`obs_effect_param_has_cols`],
//! [`obs_effect_param_cols_bad`] (then adds the column error) and
//! [`obs_effect_param_cols`], each where it read that byte;
//! `ep_parse_param_assign_intfloat()` asks [`obs_effect_param_int_value`]
//! and `ep_parse_param_array()` [`obs_effect_param_array_count`]. It keeps
//! every token call, every `cf_adderror()` and the float values. Only a C
//! string, a length, `bool`s, `int`s and an `int` out-pointer cross; the
//! route is a C `int`. The shims take no lock, call nothing in libobs and
//! stay hidden (`rust-hidden.d/obs-graphics-effect_param_types.txt`).
//!
//! The number token's text runs on into the effect source: the number shims
//! hand the core one byte at a time, which it reads once each in the C's
//! order (`valid_int_str()` first for a count, then `strtol()`), so they
//! read no byte the C would not.
//!
//! One intentional difference (#3464): the int value and the array count
//! saturate to `int` past its range, where the C depends on the width of
//! `long`. A null name, which `effect-parser.c` never passes (the C would
//! call `strcmp(NULL)`), reads as `""`, and a null number text as `""` and
//! false.

use core::ffi::{CStr, c_char, c_int};

use crate::effect_param_types as core_types;

/// The bytes of `type_`, without its terminator; `""` for null.
///
/// # Safety
///
/// `type_` is null or a valid NUL-terminated string.
unsafe fn name<'a>(type_: *const c_char) -> &'a [u8] {
    if type_.is_null() {
        return b"";
    }
    // SAFETY: the caller passes a valid NUL-terminated string.
    unsafe { CStr::from_ptr(type_) }.to_bytes()
}

/// `enum gs_shader_param_type obs_effect_param_type(const char *type)`.
///
/// # Safety
///
/// `type_` must be null or a valid NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_effect_param_type(type_: *const c_char) -> c_int {
    // SAFETY: forwarded from the caller.
    core_types::param_type(unsafe { name(type_) })
}

/// `enum obs_effect_param_route obs_effect_param_route(const char *type,
/// bool is_texture)`.
///
/// # Safety
///
/// `type_` must be null or a valid NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_effect_param_route(type_: *const c_char, is_texture: bool) -> c_int {
    // SAFETY: forwarded from the caller.
    core_types::route(unsafe { name(type_) }, is_texture) as c_int
}

/// `bool obs_effect_param_rows_bad(const char *type, bool is_float)`.
///
/// # Safety
///
/// `type_` must be null or a valid NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_effect_param_rows_bad(type_: *const c_char, is_float: bool) -> bool {
    // SAFETY: forwarded from the caller.
    core_types::rows_bad(unsafe { name(type_) }, is_float)
}

/// `int obs_effect_param_rows(const char *type, bool is_float)`.
///
/// # Safety
///
/// `type_` must be null or a valid NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_effect_param_rows(type_: *const c_char, is_float: bool) -> c_int {
    // SAFETY: forwarded from the caller.
    core_types::rows(unsafe { name(type_) }, is_float)
}

/// `bool obs_effect_param_has_cols(const char *type, bool is_float)`.
///
/// # Safety
///
/// `type_` must be null or a valid NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_effect_param_has_cols(type_: *const c_char, is_float: bool) -> bool {
    // SAFETY: forwarded from the caller.
    core_types::has_cols(unsafe { name(type_) }, is_float)
}

/// `bool obs_effect_param_cols_bad(const char *type, bool is_float)`.
///
/// # Safety
///
/// `type_` must be null or a valid NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_effect_param_cols_bad(type_: *const c_char, is_float: bool) -> bool {
    // SAFETY: forwarded from the caller.
    core_types::cols_bad(unsafe { name(type_) }, is_float)
}

/// `int obs_effect_param_cols(const char *type, bool is_float, int count)`.
///
/// # Safety
///
/// `type_` must be null or a valid NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_effect_param_cols(
    type_: *const c_char,
    is_float: bool,
    count: c_int,
) -> c_int {
    // SAFETY: forwarded from the caller.
    core_types::cols(unsafe { name(type_) }, is_float, count)
}

/// Byte `i` of `str`, for the core's one-pass reads.
///
/// # Safety
///
/// `str` is a valid NUL-terminated string, and no byte before `i` is NUL
/// (the core asks a byte only after reading every one before it).
unsafe fn at(str: *const c_char, i: usize) -> u8 {
    // SAFETY: from the caller; the string runs at least to byte `i`. Reading
    // through `*const u8` reinterprets the same byte unsigned whatever the
    // signedness of `c_char` (`i8` on x86_64, `u8` on aarch64), so it compiles
    // on both, unlike `cast_unsigned` or an `as u8` cast.
    unsafe { *str.cast::<u8>().add(i) }
}

/// `int obs_effect_param_int_value(const char *str, bool negative)`.
///
/// Intentional difference (#3464): saturated to `int` on every platform.
///
/// # Safety
///
/// `str` must be null or a valid NUL-terminated string; null reads as `""`
/// (the C would pass it to `strtol()`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_effect_param_int_value(str: *const c_char, negative: bool) -> c_int {
    if str.is_null() {
        return core_types::int_value(b"", negative);
    }
    // SAFETY: forwarded from the caller; the core reads in order and stops
    // at a NUL.
    core_types::int_value_by(|i| unsafe { at(str, i) }, negative)
}

/// `bool obs_effect_param_array_count(const char *str, size_t len, int
/// *count)`: `*count` is written only when it returns true.
///
/// Intentional difference (#3464): the count is saturated to `int` on every
/// platform.
///
/// # Safety
///
/// `str` must be null or a valid NUL-terminated string, and `count` valid
/// for a write when `str` passes; null `str` returns false, as
/// `valid_int_str(NULL, len)` does.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_effect_param_array_count(
    str: *const c_char,
    len: usize,
    count: *mut c_int,
) -> bool {
    if str.is_null() {
        return false;
    }
    // SAFETY: forwarded from the caller; the core reads in order and stops
    // at a NUL.
    let Some(n) = core_types::array_count_by(|i| unsafe { at(str, i) }, len) else {
        return false;
    };
    // SAFETY: the caller passes a valid `count` for a valid count.
    unsafe { count.write(n) };
    true
}
