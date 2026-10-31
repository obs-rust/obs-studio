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
//! The five dimension functions are each compared on their own, the
//! column ones for every name long enough to hold the byte after the row
//! digit, with the row count and counts of 0, +-1 and +-1000.
//!
//! No intentional difference. Left out, as the C never meets them: the
//! dimensions of a name no longer than its prefix, and the column of a name
//! that ends at the row digit (the C reads past the terminator; the route
//! never gives the first, and the parser asks the column only after an
//! `x`), a count whose product overflows `int`, and a null name (the C calls
//! `strcmp(NULL)`; the shim reads it as `""`, checked on the Rust side).
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use std::ffi::{CString, c_int};
use std::ptr;

use obs_c_oracle::effect_param_types::{self as c, layout};
use obs_c_oracle::parity;
use obs_graphics::effect_param_types::{
    Route, cols, cols_bad, has_cols, param_type, route, rows, rows_bad,
};
use obs_graphics::ffi::effect_param_types as rs;
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
        let at = |what: &str| format!("{what} of {shown:?} ({is_float})");
        // SAFETY: as above, and `name` is longer than the prefix, so the row
        // digit and the byte after it are within the string.
        unsafe {
            let c = c::oracle_obs_effect_param_rows_bad(p, is_float);
            assert_eq!(
                rs::obs_effect_param_rows_bad(p, is_float),
                c,
                "{}",
                at("rows_bad")
            );
            assert_eq!(rows_bad(name, is_float), c, "{}", at("core rows_bad"));
            let c = c::oracle_obs_effect_param_rows(p, is_float);
            assert_eq!(rs::obs_effect_param_rows(p, is_float), c, "{}", at("rows"));
            assert_eq!(rows(name, is_float), c, "{}", at("core rows"));
            let c = c::oracle_obs_effect_param_has_cols(p, is_float);
            assert_eq!(
                rs::obs_effect_param_has_cols(p, is_float),
                c,
                "{}",
                at("has_cols")
            );
            assert_eq!(has_cols(name, is_float), c, "{}", at("core has_cols"));
        }
        if name.len() <= prefix + 1 {
            continue; // the column digit would be past the terminator
        }
        let row = rows(name, is_float);
        // SAFETY: as above; the column digit is within the string or its
        // terminator, and every count keeps the C product within int.
        unsafe {
            let c = c::oracle_obs_effect_param_cols_bad(p, is_float);
            assert_eq!(
                rs::obs_effect_param_cols_bad(p, is_float),
                c,
                "{}",
                at("cols_bad")
            );
            assert_eq!(cols_bad(name, is_float), c, "{}", at("core cols_bad"));
            for count in [row, 0, 1, -1, 1000, -1000] {
                let c = c::oracle_obs_effect_param_cols(p, is_float, count);
                assert_eq!(
                    rs::obs_effect_param_cols(p, is_float, count),
                    c,
                    "{}",
                    at("cols")
                );
                assert_eq!(cols(name, is_float, count), c, "{}", at("core cols"));
            }
        }
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

    assert_eq!(layout_value(layout::BOOL_SIZE), size_of::<bool>());
    assert_eq!(layout_value(layout::BOOL_ALIGN), align_of::<bool>());
    assert_eq!(layout_value(layout::INT_SIZE), size_of::<c_int>());
    assert_eq!(layout_value(layout::INT_ALIGN), align_of::<c_int>());
    assert_eq!(layout_value(layout::ROUTE_SIZE), size_of::<c_int>());
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
        for is_float in [false, true] {
            let n = ptr::null();
            assert_eq!(
                rs::obs_effect_param_rows_bad(n, is_float),
                rows_bad(b"", is_float)
            );
            assert_eq!(rs::obs_effect_param_rows(n, is_float), rows(b"", is_float));
            assert_eq!(
                rs::obs_effect_param_has_cols(n, is_float),
                has_cols(b"", is_float)
            );
            assert_eq!(
                rs::obs_effect_param_cols_bad(n, is_float),
                cols_bad(b"", is_float)
            );
            assert_eq!(
                rs::obs_effect_param_cols(n, is_float, 3),
                cols(b"", is_float, 3)
            );
        }
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
