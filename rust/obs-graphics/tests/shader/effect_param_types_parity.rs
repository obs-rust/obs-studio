//! Tier 3: the Rust shims and the core answer exactly like the original
//! `libobs/graphics/effect-param-types.c.h`, compiled as `oracle_*` with the
//! C `dstr.c`.
//!
//! The decisions read a known prefix (at most `float4x4`, eight bytes, or
//! `texture`, seven) and at most three bytes after `int` or `float`, so
//! every name of up to four bytes over the bytes that matter, and every
//! known name followed by every tail of up to three of them, reach every
//! path. The bytes that matter are the letters of the known names, the
//! digits, `x`, `_` and one non-ASCII byte (`0xc3`, negative as a signed C
//! `char`, which checks the core's `c_char` arithmetic on each target).
//! Every known name with each one-byte suffix and each one-byte cut, and
//! proptests of any bytes, cover the rest.
//!
//! No intentional difference. Left out, as the C never meets them: the
//! shape of a name no longer than its prefix (the C reads past the
//! terminator; the route never gives one) and a null name (the C calls
//! `strcmp(NULL)`; the shim reads it as `""`, checked on the Rust side).
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use std::ffi::{CString, c_int};
use std::ptr;

use obs_c_oracle::effect_param_types::{self as c, layout};
use obs_c_oracle::parity;
use obs_graphics::effect_param_types::{Route, array_shape, param_type, route};
use obs_graphics::ffi::effect_param_types::{self as rs, obs_effect_param_shape};
use obs_graphics::shader_param_names::param;
use obs_util as _; // bmem for the C dstr.c
use proptest::prelude::*;

/// The known names and the prefixes the decisions test.
const KNOWN: &[&str] = &[
    "",
    "float",
    "float2",
    "float3",
    "float4",
    "float4x4",
    "int",
    "int2",
    "int3",
    "int4",
    "bool",
    "string",
    "texture",
    "texture2d",
    "texture_rect",
    "float3x3",
    "int1",
    "strings",
];

/// The bytes that matter: the known names' letters, the digits, `x`, `_`
/// and one non-ASCII byte.
const ALPHABET: &[u8] = b"abdefgilnorstux0123456789_\xc3";

fn layout_value(which: usize) -> usize {
    // SAFETY: plain value lookup; out-of-range indices return 0.
    unsafe { c::oracle_effect_param_types_layout(which) }
}

/// Every answer for `name`, from the oracle, the shim and the core.
fn check(name: &[u8]) {
    let cs = CString::new(name).unwrap();
    let p = cs.as_ptr();
    let shown = String::from_utf8_lossy(name);

    // SAFETY: `p` is a valid NUL-terminated string for every call below.
    let (c_type, rs_type) = unsafe {
        (
            c::oracle_obs_effect_param_type(p),
            rs::obs_effect_param_type(p),
        )
    };
    assert_eq!(rs_type, c_type, "type of {shown:?}");
    assert_eq!(param_type(name), c_type, "core type of {shown:?}");

    for is_texture in [false, true] {
        // SAFETY: as above.
        let (c_route, rs_route) = unsafe {
            (
                c::oracle_obs_effect_param_route(p, is_texture),
                rs::obs_effect_param_route(p, is_texture),
            )
        };
        assert_eq!(rs_route, c_route, "route of {shown:?} ({is_texture})");
        assert_eq!(
            route(name, is_texture) as c_int,
            c_route,
            "core route of {shown:?}"
        );
    }

    for (is_float, prefix) in [(false, 3), (true, 5)] {
        if name.len() <= prefix {
            continue; // the C reads past the terminator
        }
        // SAFETY: as above, and `name` is longer than the prefix, so the C
        // reads only within the string.
        let (cv, rv) = unsafe {
            (
                c::oracle_obs_effect_param_array_shape(p, is_float),
                rs::obs_effect_param_array_shape(p, is_float),
            )
        };
        let rv = (rv.count, rv.bad_rows, rv.bad_cols);
        assert_eq!(
            rv,
            (cv.count, cv.bad_rows, cv.bad_cols),
            "shape of {shown:?} ({is_float})"
        );
        let s = array_shape(name, is_float);
        assert_eq!(
            (s.count, s.bad_rows, s.bad_cols),
            rv,
            "core shape of {shown:?}"
        );
    }
}

/// Every name of `len` bytes over `ALPHABET` after `prefix`, the `i`-th of
/// them.
fn name_at(prefix: &[u8], len: u32, mut i: u64) -> Vec<u8> {
    let mut name = prefix.to_vec();
    let n = ALPHABET.len() as u64;
    for _ in 0..len {
        name.push(ALPHABET[(i % n) as usize]);
        i /= n;
    }
    name
}

fn every_name(prefix: &[u8], max_len: u32) {
    let n = ALPHABET.len() as u64;
    for len in 0..=max_len {
        let total = n.pow(len);
        for i in 0..total {
            // The one- and two-byte tails on every level; longer ones
            // sampled below the full level.
            if len <= 2 || parity::keep(i) {
                check(&name_at(prefix, len, i));
            }
        }
    }
}

#[test]
fn enums_and_layout() {
    for (i, v) in [
        param::UNKNOWN,
        param::BOOL,
        param::FLOAT,
        param::INT,
        param::STRING,
        param::VEC2,
        param::VEC3,
        param::VEC4,
        param::INT2,
        param::INT3,
        param::INT4,
        param::MATRIX4X4,
        param::TEXTURE,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            layout_value(layout::PARAM_TYPES + i),
            v as usize,
            "param type {i}"
        );
    }
    assert_eq!(layout::PARAM_TYPE_COUNT, 13);
    for (i, r) in [
        Route::Texture,
        Route::Int,
        Route::Float,
        Route::IntArray,
        Route::FloatArray,
        Route::String,
        Route::Bool,
        Route::Invalid,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(layout_value(layout::ROUTES + i), r as usize, "route {i}");
    }
    assert_eq!(layout::ROUTE_COUNT, 8);

    assert_eq!(
        layout_value(layout::SHAPE_SIZE),
        size_of::<obs_effect_param_shape>()
    );
    assert_eq!(
        layout_value(layout::SHAPE_ALIGN),
        align_of::<obs_effect_param_shape>()
    );
    assert_eq!(
        layout_value(layout::SHAPE_COUNT),
        std::mem::offset_of!(obs_effect_param_shape, count)
    );
    assert_eq!(
        layout_value(layout::SHAPE_BAD_ROWS),
        std::mem::offset_of!(obs_effect_param_shape, bad_rows)
    );
    assert_eq!(
        layout_value(layout::SHAPE_BAD_COLS),
        std::mem::offset_of!(obs_effect_param_shape, bad_cols)
    );
    assert_eq!(
        size_of::<c::OracleShape>(),
        size_of::<obs_effect_param_shape>()
    );
}

#[test]
fn every_short_name() {
    every_name(b"", 4);
}

#[test]
fn every_known_name_with_every_tail() {
    for k in KNOWN {
        every_name(k.as_bytes(), 3);
    }
}

#[test]
fn every_known_name_with_each_byte() {
    for k in KNOWN {
        let k = k.as_bytes();
        for b in 1..=u8::MAX {
            check(&[k, &[b]].concat());
            check(&[&[b], k].concat());
        }
        for cut in 0..k.len() {
            let mut n = k.to_vec();
            n.remove(cut);
            check(&n);
            check(&k[..cut]);
        }
    }
}

#[test]
fn every_byte_as_a_digit() {
    // The row and the column digit over every byte, with and without the
    // 'x', after both prefixes.
    for prefix in [b"int".as_slice(), b"float"] {
        for b in 1..=u8::MAX {
            check(&[prefix, &[b]].concat());
            check(&[prefix, &[b, b'x']].concat());
            check(&[prefix, &[b'2', b'x', b]].concat());
            check(&[prefix, &[b, b'x', b]].concat());
        }
    }
}

#[test]
fn a_null_name_reads_as_empty() {
    // The C calls strcmp(NULL) here; only the Rust side is checked.
    // SAFETY: the shims take null.
    unsafe {
        assert_eq!(rs::obs_effect_param_type(ptr::null()), param_type(b""));
        assert_eq!(
            rs::obs_effect_param_route(ptr::null(), false),
            Route::Invalid as c_int
        );
        assert_eq!(
            rs::obs_effect_param_route(ptr::null(), true),
            Route::Texture as c_int
        );
        let s = rs::obs_effect_param_array_shape(ptr::null(), true);
        let e = array_shape(b"", true);
        assert_eq!(
            (s.count, s.bad_rows, s.bad_cols),
            (e.count, e.bad_rows, e.bad_cols)
        );
    }
}

/// Any name without a NUL.
fn any_name() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(1..=u8::MAX, 0..12)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(parity::cases(512)))]

    #[test]
    fn any_bytes(n in any_name()) {
        check(&n);
    }

    #[test]
    fn any_bytes_after_a_known_name(k in prop::sample::select(KNOWN), t in any_name()) {
        check(&[k.as_bytes(), &t].concat());
    }
}
