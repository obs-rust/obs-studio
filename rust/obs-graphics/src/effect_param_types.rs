//! Safe core of `libobs/graphics/effect-param-types.c.h`: the parameter
//! type decisions of the effect parser (`effect-parser.c`), on a type name.
//!
//! - [`param_type`] (`get_effect_param_type`, for `ep_compile_annotations`
//!   and `ep_compile_param`): the shader parameter type of a type name. It
//!   is the map of `get_shader_param_type`, so it calls
//!   [`shader_param_type`](crate::shader_param_names::shader_param_type).
//! - [`route`] (`ep_parse_param_assignment_val`): the default-value parser
//!   a parameter gets.
//! - [`array_shape`] (`ep_parse_param_assign_intfloat_array`): the value
//!   count of an `int` or `float` array type, and whether its row or column
//!   digit is outside 1-4 (`effect-parser.c` adds each error).
//!
//! Names compare as bytes, case-sensitively, as `strcmp` and `astrcmp_n`
//! do. `effect-parser.c` keeps every token call, every `cf_adderror()`, the
//! value parsers, the brace and constructor handling, the loop over the
//! values and the compile. Nothing here keeps state or calls libobs.
//!
//! Characterized, not endorsed (`test_effect_param_types_steps`), and kept:
//! any name starting with `texture` is a texture; `strings` takes the string
//! parser and `int1` or `float3x3` array values, with an unknown type; a bad
//! row or column digit still counts, as a C `char` minus `'0'` (`floatx` is
//! 72 values, the terminator after an `x` is `-48`). The digit is a
//! [`c_char`], so a non-ASCII byte counts as the C does on each target:
//! negative where `char` is signed (x86-64, MSVC), not on arm64 Linux.
//!
//! No intentional difference from the C.

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

/// The values of an `int` or `float` array type
/// (`struct obs_effect_param_shape`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Shape {
    /// The row digit, times the column digit after an `x`, each a C `char`
    /// minus `'0'`: zero, negative or large after an error.
    pub count: i32,
    /// The row digit is outside 1-4.
    pub bad_rows: bool,
    /// The column digit after an `x` is outside 1-4.
    pub bad_cols: bool,
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

/// `obs_effect_param_array_shape()`: the shape of `name` after its `float`
/// (`is_float`) or `int` prefix. A byte past the end of `name` reads as the
/// terminator, `0`.
#[must_use]
pub fn array_shape(name: &[u8], is_float: bool) -> Shape {
    let dims = name.get(if is_float { 5 } else { 3 }..).unwrap_or_default();
    let at = |i: usize| dims.get(i).copied().unwrap_or(0);

    let rows = at(0);
    let mut shape = Shape {
        count: digit(rows),
        bad_rows: !is_dim(rows),
        bad_cols: false,
    };
    // The column digit is read only after an 'x'.
    if at(1) == b'x' {
        let cols = at(2);
        shape.bad_cols = !is_dim(cols);
        // Each digit is within -176..=207, so the product fits.
        shape.count = shape.count.wrapping_mul(digit(cols));
    }
    shape
}

/// A row or column digit, `'1'` to `'4'`.
fn is_dim(b: u8) -> bool {
    (b'1'..=b'4').contains(&b)
}

/// `b - '0'` with `b` a C `char`: signed or not as on the target.
fn digit(b: u8) -> i32 {
    i32::from(b as c_char).wrapping_sub(i32::from(b'0'))
}
