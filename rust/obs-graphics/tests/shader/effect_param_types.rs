//! Tier 1: the safe core of `libobs/graphics/effect-param-types.c.h`.
//!
//! `steps_*` mirror `test/cmocka/test_effect_param_types_steps.c` one to
//! one (same names, values and expectations). `edge_*` cover what the C
//! test leaves out: non-ASCII digit bytes (as a C `char` on this target), a
//! name that ends at the row digit or right after an `x`, and proptests of
//! every decision against a model.

use core::ffi::c_char;

use obs_graphics::effect_param_types::{Route, Shape, array_shape, param_type, route};
use obs_graphics::shader_param_names::{param, shader_param_type};
use proptest::prelude::*;

fn ty(name: &str) -> i32 {
    param_type(name.as_bytes())
}

fn rt(name: &str, is_texture: bool) -> Route {
    route(name.as_bytes(), is_texture)
}

fn check_shape(name: &str, is_float: bool, count: i32, bad_rows: bool, bad_cols: bool) {
    assert_eq!(
        array_shape(name.as_bytes(), is_float),
        Shape {
            count,
            bad_rows,
            bad_cols,
        },
        "{name}"
    );
}

/// A byte as the C `char` it is on this target, minus `'0'`.
fn digit(b: u8) -> i32 {
    i32::from(b as c_char) - i32::from(b'0')
}

#[test]
fn steps_every_exact_type_name() {
    assert_eq!(ty("float"), param::FLOAT);
    assert_eq!(ty("float2"), param::VEC2);
    assert_eq!(ty("float3"), param::VEC3);
    assert_eq!(ty("float4"), param::VEC4);
    assert_eq!(ty("int2"), param::INT2);
    assert_eq!(ty("int3"), param::INT3);
    assert_eq!(ty("int4"), param::INT4);
    assert_eq!(ty("float4x4"), param::MATRIX4X4);
    assert_eq!(ty("bool"), param::BOOL);
    assert_eq!(ty("int"), param::INT);
    assert_eq!(ty("string"), param::STRING);
}

#[test]
fn steps_any_texture_prefix_is_a_texture() {
    assert_eq!(ty("texture"), param::TEXTURE);
    assert_eq!(ty("texture2d"), param::TEXTURE);
    assert_eq!(ty("texture3d"), param::TEXTURE);
    assert_eq!(ty("texture_rect"), param::TEXTURE);
    // Characterized, not endorsed: any name that starts with it.
    assert_eq!(ty("textures"), param::TEXTURE);
    assert_eq!(ty("texturefloat4"), param::TEXTURE);
    // A cut of the prefix is not.
    assert_eq!(ty("textur"), param::UNKNOWN);
}

#[test]
fn steps_other_type_names_are_unknown() {
    assert_eq!(ty(""), param::UNKNOWN);
    // Case matters.
    assert_eq!(ty("Float"), param::UNKNOWN);
    assert_eq!(ty("BOOL"), param::UNKNOWN);
    assert_eq!(ty("Texture2d"), param::UNKNOWN);
    // Exact names only: a suffix or a cut is unknown.
    assert_eq!(ty("floats"), param::UNKNOWN);
    assert_eq!(ty("float5"), param::UNKNOWN);
    assert_eq!(ty("floa"), param::UNKNOWN);
    assert_eq!(ty("int1"), param::UNKNOWN);
    assert_eq!(ty("float3x3"), param::UNKNOWN);
    assert_eq!(ty("float4x4 "), param::UNKNOWN);
    assert_eq!(ty("strings"), param::UNKNOWN);
    assert_eq!(ty("sampler"), param::UNKNOWN);
}

#[test]
fn steps_every_route() {
    assert_eq!(rt("int", false), Route::Int);
    assert_eq!(rt("float", false), Route::Float);
    assert_eq!(rt("int2", false), Route::IntArray);
    assert_eq!(rt("int4", false), Route::IntArray);
    assert_eq!(rt("float2", false), Route::FloatArray);
    assert_eq!(rt("float4x4", false), Route::FloatArray);
    assert_eq!(rt("string", false), Route::String);
    assert_eq!(rt("bool", false), Route::Bool);
    assert_eq!(rt("", false), Route::Invalid);
    assert_eq!(rt("sampler", false), Route::Invalid);
    assert_eq!(rt("Float", false), Route::Invalid);
    assert_eq!(rt("bools", false), Route::Invalid);
    assert_eq!(rt("in", false), Route::Invalid);
}

#[test]
fn steps_the_texture_flag_comes_first() {
    // The flag decides, whatever the name.
    assert_eq!(rt("texture2d", true), Route::Texture);
    assert_eq!(rt("int", true), Route::Texture);
    assert_eq!(rt("bool", true), Route::Texture);
    assert_eq!(rt("", true), Route::Texture);
    // Without it a texture name has no default-value parser.
    assert_eq!(rt("texture2d", false), Route::Invalid);
}

#[test]
fn steps_routes_that_disagree_with_the_type() {
    // Characterized, not endorsed: the string parser, but an unknown type.
    assert_eq!(rt("strings", false), Route::String);
    assert_eq!(ty("strings"), param::UNKNOWN);
    // Characterized, not endorsed: array values, but an unknown type.
    assert_eq!(rt("int1", false), Route::IntArray);
    assert_eq!(ty("int1"), param::UNKNOWN);
    assert_eq!(rt("float3x3", false), Route::FloatArray);
    assert_eq!(ty("float3x3"), param::UNKNOWN);
    // Characterized, not endorsed: any int or float prefix is an array.
    assert_eq!(rt("integer", false), Route::IntArray);
    assert_eq!(rt("floatx", false), Route::FloatArray);
}

#[test]
fn steps_valid_shapes() {
    check_shape("float2", true, 2, false, false);
    check_shape("float4", true, 4, false, false);
    check_shape("int1", false, 1, false, false);
    check_shape("int3", false, 3, false, false);
    check_shape("float4x4", true, 16, false, false);
    check_shape("float3x2", true, 6, false, false);
    check_shape("int1x4", false, 4, false, false);
    // Only the byte after the row digit is looked at for an 'x'.
    check_shape("float4y4", true, 4, false, false);
    check_shape("float4x4x4", true, 16, false, false);
}

#[test]
fn steps_bad_row_digits() {
    // Characterized, not endorsed: the error is added, the digit still
    // counts.
    check_shape("float5", true, 5, true, false);
    check_shape("float0", true, 0, true, false);
    check_shape("int9", false, 9, true, false);
    check_shape("float0x4", true, 0, true, false);
    // Characterized, not endorsed: 'x' - '0'.
    check_shape("floatx", true, 72, true, false);
    check_shape("integer", false, i32::from(b'e') - 48, true, false);
    check_shape("float_", true, i32::from(b'_') - 48, true, false);
}

#[test]
fn steps_bad_column_digits() {
    // Characterized, not endorsed: the error is added, the digit still
    // counts, also as the terminator: '\0' - '0'.
    check_shape("float4x", true, 4 * -48, false, true);
    check_shape("float4x9", true, 36, false, true);
    check_shape("float4x0", true, 0, false, true);
    check_shape("int2x5", false, 10, false, true);
    check_shape("float5x5", true, 25, true, true);
    check_shape("floatxx", true, 72 * -48, true, true);
    check_shape("floatxxx", true, 72 * 72, true, true);
}

#[test]
fn edge_non_ascii_digits_count_as_a_c_char() {
    for b in [0x80_u8, 0xc3, 0xff] {
        let row = [b"float".as_slice(), &[b]].concat();
        assert_eq!(
            array_shape(&row, true),
            Shape {
                count: digit(b),
                bad_rows: true,
                bad_cols: false,
            }
        );
        let col = [b"int2x".as_slice(), &[b]].concat();
        assert_eq!(
            array_shape(&col, false),
            Shape {
                count: 2 * digit(b),
                bad_rows: false,
                bad_cols: true,
            }
        );
        let both = [b"float".as_slice(), &[b, b'x', b]].concat();
        assert_eq!(
            array_shape(&both, true),
            Shape {
                count: digit(b) * digit(b),
                bad_rows: true,
                bad_cols: true,
            }
        );
    }
}

#[test]
fn edge_the_name_ends_early() {
    // A byte past the end reads as the terminator. The route never gives
    // the bare prefix, where the C would read past it.
    check_shape("float", true, -48, true, false);
    check_shape("int", false, -48, true, false);
    check_shape("int4x", false, 4 * -48, false, true);
    // The prefix length follows is_float, not the name.
    check_shape("float4", false, i32::from(b'a') - 48, true, false);
    check_shape("int22", true, -48, true, false);
}

#[test]
fn edge_route_order() {
    // Exact int and float come before their prefixes, the string prefix
    // before exact bool.
    assert_eq!(rt("int", false), Route::Int);
    assert_eq!(rt("int ", false), Route::IntArray);
    assert_eq!(rt("float", false), Route::Float);
    assert_eq!(rt("floats", false), Route::FloatArray);
    assert_eq!(rt("stringbool", false), Route::String);
    assert_eq!(rt("strin", false), Route::Invalid);
    assert_eq!(rt("boo", false), Route::Invalid);
    assert_eq!(rt("texture", false), Route::Invalid);
}

/// `route` as the C reads it.
fn model_route(n: &[u8], is_texture: bool) -> Route {
    if is_texture {
        Route::Texture
    } else if n == b"int" {
        Route::Int
    } else if n == b"float" {
        Route::Float
    } else if n.starts_with(b"int") {
        Route::IntArray
    } else if n.starts_with(b"float") {
        Route::FloatArray
    } else if n.starts_with(b"string") {
        Route::String
    } else if n == b"bool" {
        Route::Bool
    } else {
        Route::Invalid
    }
}

/// `array_shape` as the C reads it.
fn model_shape(n: &[u8], is_float: bool) -> Shape {
    let at = |i: usize| n.get(i).copied().unwrap_or(0);
    let p = if is_float { 5 } else { 3 };
    let d0 = at(p);
    let mut s = Shape {
        count: digit(d0),
        bad_rows: !(b'1'..=b'4').contains(&d0),
        bad_cols: false,
    };
    if at(p + 1) == b'x' {
        let d2 = at(p + 2);
        s.bad_cols = !(b'1'..=b'4').contains(&d2);
        s.count *= digit(d2);
    }
    s
}

/// Names built from the known names and the bytes that matter.
fn name() -> impl Strategy<Value = Vec<u8>> {
    let known = prop::sample::select(vec![
        "", "float", "float2", "float4x4", "int", "int3", "string", "bool", "texture",
    ]);
    let tail = prop::collection::vec(
        prop::sample::select(b"x_0123456789fintsbo\x80\xc3\xff".to_vec()),
        0..4,
    );
    (known, tail).prop_map(|(k, t)| [k.as_bytes(), &t].concat())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(512)))]

    #[test]
    fn edge_param_type_is_the_shader_map(n in name()) {
        prop_assert_eq!(param_type(&n), shader_param_type(&n));
    }

    #[test]
    fn edge_route_matches_the_model(n in name(), is_texture: bool) {
        prop_assert_eq!(route(&n, is_texture), model_route(&n, is_texture));
    }

    #[test]
    fn edge_array_shape_matches_the_model(n in name(), is_float: bool) {
        prop_assert_eq!(array_shape(&n, is_float), model_shape(&n, is_float));
    }
}
