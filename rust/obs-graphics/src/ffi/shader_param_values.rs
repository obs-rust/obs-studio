//! C ABI for `libobs/graphics/shader-param-values.c.h`, the number reads of
//! the shader parser, declared in `libobs/graphics/shader-param-values.h`
//! without `EXPORT`.
//!
//! `shader-parser.c` (still C) calls these where it read each number:
//! `sp_parse_param_array()` asks [`shader_param_array_count`] and
//! `sp_parse_param_assign_intfloat()` asks [`shader_param_int_value`] or
//! [`shader_param_float_value`]. It keeps every token call, every
//! `cf_adderror()` and the push of each value. Only a C string, a length, a
//! `bool`, an `int` out-pointer, an `int` and a `float` cross. The shims take
//! no lock, call nothing in libobs and stay hidden
//! (`rust-hidden.d/obs-graphics-shader_param_values.txt`).
//!
//! The number token's text runs on into the shader source: the shims hand
//! the core one byte at a time, which it reads once each in the C's order
//! (`valid_int_str()` first for a count, then `strtol()`; `os_strtod()`'s
//! copy of at most 63 bytes for a float), so they read no byte the C would
//! not.
//!
//! Two intentional differences (#3468), where the C depends on the width
//! of `long`: the int value saturates to `int` past its range, the effect
//! parser's rule (#3464), and a count outside `int` returns false. A null text, which `shader-parser.c`
//! never passes (the C would hand it to `strtol()` or `os_strtod()`), reads
//! as `""`, and a null count text as false.

use core::ffi::{c_char, c_float, c_int};

use crate::shader_param_values as core_values;

/// Byte `i` of `str`, for the core's one-pass reads.
///
/// # Safety
///
/// `str` is a valid NUL-terminated string, and no byte before `i` is NUL
/// (the core asks a byte only after reading every one before it).
unsafe fn at(str: *const c_char, i: usize) -> u8 {
    // SAFETY: from the caller; the string runs at least to byte `i`.
    unsafe { *str.add(i) }.cast_unsigned()
}

/// `bool shader_param_array_count(const char *str, size_t len, int
/// *count)`: `*count` is written only when it returns true.
///
/// Intentional difference (#3468): false for a count outside `int`.
///
/// # Safety
///
/// `str` must be null or a valid NUL-terminated string, and `count` valid
/// for a write when `str` passes; null `str` returns false, as
/// `valid_int_str(NULL, len)` does.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn shader_param_array_count(
    str: *const c_char,
    len: usize,
    count: *mut c_int,
) -> bool {
    if str.is_null() {
        return false;
    }
    // SAFETY: forwarded from the caller; the core reads in order and stops
    // at a NUL.
    let Some(n) = core_values::array_count_by(|i| unsafe { at(str, i) }, len) else {
        return false;
    };
    // SAFETY: the caller passes a valid `count` for a valid count.
    unsafe { count.write(n) };
    true
}

/// `int shader_param_int_value(const char *str, bool negative)`.
///
/// Intentional difference (#3468): saturated to `int` on every platform.
///
/// # Safety
///
/// `str` must be null or a valid NUL-terminated string; null reads as `""`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn shader_param_int_value(str: *const c_char, negative: bool) -> c_int {
    if str.is_null() {
        return core_values::int_value(b"", negative);
    }
    // SAFETY: forwarded from the caller; the core reads in order and stops
    // at a NUL.
    core_values::int_value_by(|i| unsafe { at(str, i) }, negative)
}

unsafe extern "C" {
    /// libobs's `os_strtod`, through the private adapter in
    /// `obs-graphics-adapter.c.h`: an executable that links the Rust
    /// staticlib next to libobs (the cmocka `*_rust` tests) then never pulls
    /// obs-util's host code, and the libraries and hidden libobs glue it
    /// needs, out of the staticlib (#3468).
    fn obs_graphics_os_strtod(str: *const c_char) -> f64;
}

/// `os_strtod` of the core's NUL-terminated copy.
fn libobs_strtod(copy: &[u8]) -> f64 {
    // SAFETY: the core passes its copy with the NUL that ends it.
    unsafe { obs_graphics_os_strtod(copy.as_ptr().cast()) }
}

/// `float shader_param_float_value(const char *str, bool negative)`.
///
/// # Safety
///
/// `str` must be null or a valid NUL-terminated string; null reads as `""`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn shader_param_float_value(str: *const c_char, negative: bool) -> c_float {
    if str.is_null() {
        return core_values::float_value(b"", negative, libobs_strtod);
    }
    // SAFETY: forwarded from the caller; the core reads in order and stops
    // at a NUL or after 63 bytes.
    core_values::float_value_by(|i| unsafe { at(str, i) }, negative, libobs_strtod)
}
