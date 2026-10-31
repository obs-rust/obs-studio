//! C ABI for `libobs/graphics/effect-param-types.c.h`, the parameter type
//! decisions of the effect parser, declared in
//! `libobs/graphics/effect-param-types.h` without `EXPORT`.
//!
//! `effect-parser.c` (still C) calls these where it made each decision:
//! `ep_compile_annotations()` and `ep_compile_param()` ask
//! [`obs_effect_param_type`], `ep_parse_param_assignment_val()` switches on
//! [`obs_effect_param_route`], and `ep_parse_param_assign_intfloat_array()`
//! asks [`obs_effect_param_rows_bad`] (then adds the row error),
//! [`obs_effect_param_rows`], [`obs_effect_param_has_cols`],
//! [`obs_effect_param_cols_bad`] (then adds the column error) and
//! [`obs_effect_param_cols`], each where it read that byte. It keeps every
//! token call, every `cf_adderror()` and the value parsers. Only a C string,
//! `bool`s and `int`s cross; the route is a C `int`. The shims take no lock,
//! call nothing in libobs and stay hidden (`rust-hidden.txt`).
//!
//! No intentional difference from C. A null name, which `effect-parser.c`
//! never passes (the C would call `strcmp(NULL)`), reads as `""`.

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
