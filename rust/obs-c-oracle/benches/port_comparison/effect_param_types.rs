//! obs-graphics: the effect parameter types
//! (`libobs/graphics/effect-param-types.c.h`). Every parameter the effect
//! parser reads asks its default-value route, an array type asks its shape,
//! and every parameter and annotation the compile writes asks its type. Each
//! side runs the parameters of a typical effect file (matrices, vectors,
//! scalars, a string, a bool and textures, with their defaults) through the
//! three decisions as `effect-parser.c` asks them, and folds the answers into
//! a checksum both must match. Nothing is allocated in the loop.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::effect_param_types::{
    oracle_obs_effect_param_array_shape, oracle_obs_effect_param_route,
    oracle_obs_effect_param_type,
};
use obs_graphics::ffi::effect_param_types::{
    obs_effect_param_array_shape, obs_effect_param_route, obs_effect_param_type,
};
use std::ffi::CStr;
use std::hint::black_box;

/// The parameters of an effect: type name and texture flag.
const PARAMS: &[(&CStr, bool)] = &[
    (c"float4x4", false),
    (c"texture2d", true),
    (c"float2", false),
    (c"float4", false),
    (c"float", false),
    (c"int", false),
    (c"int2", false),
    (c"bool", false),
    (c"string", false),
    (c"float3", false),
    (c"texture2d", true),
    (c"float3x3", false),
];

/// `OBS_EFFECT_PARAM_ROUTE_INT_ARRAY` and `_FLOAT_ARRAY`.
const INT_ARRAY: i32 = 3;
const FLOAT_ARRAY: i32 = 4;

fn fold(acc: u64, value: i32) -> u64 {
    acc.wrapping_mul(31).wrapping_add(value as u64)
}

/// The effect, from the three functions of one side.
macro_rules! effect {
    ($route:expr, $shape:expr, $ty:expr) => {{
        let mut acc = 0;
        for &(name, is_texture) in PARAMS {
            let p = black_box(name).as_ptr();
            let route = $route(p, black_box(is_texture));
            acc = fold(acc, route);
            if route == INT_ARRAY || route == FLOAT_ARRAY {
                let s = $shape(p, route == FLOAT_ARRAY);
                acc = fold(fold(fold(acc, s.count), s.bad_rows as i32), s.bad_cols as i32);
            }
            acc = fold(acc, $ty(p));
        }
        acc
    }};
}

fn rust_effect() -> u64 {
    // SAFETY: every name is a valid NUL-terminated string.
    unsafe {
        effect!(
            obs_effect_param_route,
            obs_effect_param_array_shape,
            obs_effect_param_type
        )
    }
}

fn c_effect() -> u64 {
    // SAFETY: every name is a valid NUL-terminated string longer than its
    // array prefix.
    unsafe {
        effect!(
            oracle_obs_effect_param_route,
            oracle_obs_effect_param_array_shape,
            oracle_obs_effect_param_type
        )
    }
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(criterion, "effect_param_types/effect/12_params", rust_effect, c_effect);
}
