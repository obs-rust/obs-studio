//! obs-graphics: the shader parser's number reads
//! (`libobs/graphics/shader-param-values.c.h`). Each side runs the array
//! counts, `int` defaults and `float` defaults of a typical shader (a
//! colour matrix, vectors, a kernel of weights) through the three reads as
//! `shader-parser.c` asks them, each token's text running on into the
//! source as the parser passes it, and folds the answers into a checksum
//! both must match. Nothing is allocated in the loop.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::shader_param_values::{
    oracle_shader_param_array_count, oracle_shader_param_float_value, oracle_shader_param_int_value,
};
use obs_graphics::ffi::shader_param_values::{
    shader_param_array_count, shader_param_float_value, shader_param_int_value,
};
use std::ffi::CStr;
use std::hint::black_box;

/// The float defaults of a shader, each token's text running on into the
/// source, and whether a `-` token came first.
const FLOATS: &[(&CStr, bool)] = &[
    (c"1.164384, 0.0, 1.792741, -0.972945};\nuniform float2 size;", false),
    (c"0.0, 1.792741, -0.972945};\nuniform float2 size;", false),
    (c"1.792741, -0.972945};\nuniform float2 size;", false),
    (c"0.972945};\nuniform float2 size;", true),
    (c"0.0625, 0.25, 0.375, 0.25, 0.0625};\n", false),
    (c"0.25, 0.375, 0.25, 0.0625};\n", false),
    (c"0.375, 0.25, 0.0625};\n", false),
    (c"0.25, 0.0625};\n", false),
    (c"0.0625};\n", false),
    (c"2.2;\nuniform int mode = 1;", false),
    (c"1e-3;\n", true),
];

/// The int defaults of the same shader.
const INTS: &[(&CStr, bool)] = &[(c"1;\nuniform float gamma", false), (c"8;\n", true)];

/// Its array counts: the token's text and length.
const COUNTS: &[(&CStr, usize)] = &[(c"5];\nuniform float weights", 1), (c"16];\n", 2)];

fn fold(acc: u64, value: u64) -> u64 {
    acc.wrapping_mul(31).wrapping_add(value)
}

macro_rules! numbers {
    ($float:expr, $int:expr, $count:expr) => {{
        let mut acc = 0;
        for &(text, negative) in FLOATS {
            let f: f32 = $float(black_box(text).as_ptr(), black_box(negative));
            acc = fold(acc, u64::from(f.to_bits()));
        }
        for &(text, negative) in INTS {
            let i: i32 = $int(black_box(text).as_ptr(), black_box(negative));
            acc = fold(acc, u64::from(i.cast_unsigned()));
        }
        for &(text, len) in COUNTS {
            let mut count = 0_i32;
            let valid = $count(black_box(text).as_ptr(), black_box(len), &raw mut count);
            acc = fold(fold(acc, u64::from(valid)), u64::from(count.cast_unsigned()));
        }
        acc
    }};
}

fn rust_numbers() -> u64 {
    // SAFETY: every text is a valid NUL-terminated string, and every count
    // a valid int.
    unsafe {
        numbers!(
            shader_param_float_value,
            shader_param_int_value,
            shader_param_array_count
        )
    }
}

fn c_numbers() -> u64 {
    // SAFETY: as for rust_numbers().
    unsafe {
        numbers!(
            oracle_shader_param_float_value,
            oracle_shader_param_int_value,
            oracle_shader_param_array_count
        )
    }
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(
        criterion,
        "shader_param_values/numbers/15_values",
        rust_numbers,
        c_numbers,
    );
}
