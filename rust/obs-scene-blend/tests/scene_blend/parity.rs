//! Tier 3: the Rust `obs_blend_mode_params` against the original
//! `obs-scene-blend.c.h`, plus the Tier 2 layout of its row struct.
//!
//! No intentional differences. The domain is finite, so every row and every
//! enum value is compared, not sampled. A mode past `OBS_BLEND_DARKEN`
//! indexes past the end of the table in both (C undefined behaviour,
//! characterized in `test_scene_blend`); it is not read here.

use core::ffi::c_int;
use core::mem::{align_of, offset_of, size_of};
use core::slice;

use obs_c_oracle::scene_blend::{
    ENUM_BLEND_OP, ENUM_BLEND_TYPE, ENUM_BLENDING_TYPE, OracleBlendModeParam,
    oracle_scene_blend_enum, oracle_scene_blend_layout, oracle_scene_blend_rows,
    oracle_scene_blend_table,
};
use obs_scene_blend::ffi::scene_blend::{obs_blend_mode_param, obs_blend_mode_params};
use obs_scene_blend::{BlendMode, BlendOp, BlendType, MODE_COUNT, blend_params};

const TYPES: [BlendType; 11] = [
    BlendType::Zero,
    BlendType::One,
    BlendType::SrcColor,
    BlendType::InvSrcColor,
    BlendType::SrcAlpha,
    BlendType::InvSrcAlpha,
    BlendType::DstColor,
    BlendType::InvDstColor,
    BlendType::DstAlpha,
    BlendType::InvDstAlpha,
    BlendType::SrcAlphaSat,
];

const OPS: [BlendOp; 5] = [
    BlendOp::Add,
    BlendOp::Subtract,
    BlendOp::ReverseSubtract,
    BlendOp::Min,
    BlendOp::Max,
];

fn oracle_table() -> &'static [OracleBlendModeParam] {
    // SAFETY: the oracle returns its static table and its row count.
    unsafe { slice::from_raw_parts(oracle_scene_blend_table(), oracle_scene_blend_rows()) }
}

/// Every value of C enum `kind`, in declaration order.
fn oracle_enum(kind: c_int) -> Vec<c_int> {
    // SAFETY: a pure lookup; -1 marks the end.
    (0..)
        .map(|i| unsafe { oracle_scene_blend_enum(kind, i) })
        .take_while(|&v| v != -1)
        .collect()
}

fn as_oracle(p: obs_blend_mode_param) -> OracleBlendModeParam {
    OracleBlendModeParam {
        src_color: p.src_color,
        src_alpha: p.src_alpha,
        dst_color: p.dst_color,
        dst_alpha: p.dst_alpha,
        op: p.op,
    }
}

#[test]
fn parity_row_layout_matches_c() {
    let rust = [
        size_of::<obs_blend_mode_param>(),
        align_of::<obs_blend_mode_param>(),
        offset_of!(obs_blend_mode_param, src_color),
        offset_of!(obs_blend_mode_param, src_alpha),
        offset_of!(obs_blend_mode_param, dst_color),
        offset_of!(obs_blend_mode_param, dst_alpha),
        offset_of!(obs_blend_mode_param, op),
    ];
    for (i, want) in rust.into_iter().enumerate() {
        // SAFETY: a pure lookup.
        assert_eq!(
            unsafe { oracle_scene_blend_layout(i) },
            want,
            "layout entry {i}"
        );
    }
    // SAFETY: a pure lookup.
    assert_eq!(unsafe { oracle_scene_blend_layout(rust.len()) }, usize::MAX);
    assert_eq!(
        size_of_val(&obs_blend_mode_params),
        MODE_COUNT * size_of::<obs_blend_mode_param>()
    );
}

#[test]
fn parity_enum_values_match_c() {
    let modes: Vec<c_int> = BlendMode::ALL.iter().map(|&m| m as c_int).collect();
    let types: Vec<c_int> = TYPES.iter().map(|&t| t as c_int).collect();
    let ops: Vec<c_int> = OPS.iter().map(|&o| o as c_int).collect();
    assert_eq!(
        oracle_enum(ENUM_BLENDING_TYPE),
        modes,
        "enum obs_blending_type"
    );
    assert_eq!(oracle_enum(ENUM_BLEND_TYPE), types, "enum gs_blend_type");
    assert_eq!(oracle_enum(ENUM_BLEND_OP), ops, "enum gs_blend_op_type");
}

#[test]
fn parity_row_count_matches_c() {
    // SAFETY: a pure lookup.
    assert_eq!(unsafe { oracle_scene_blend_rows() }, MODE_COUNT);
    assert_eq!(obs_blend_mode_params.len(), MODE_COUNT);
}

/// The table libobs-with-rust links, row by row and byte for byte.
#[test]
fn parity_every_row_matches_c() {
    let c = oracle_table();
    assert_eq!(c.len(), obs_blend_mode_params.len());
    for (mode, (rust, c)) in BlendMode::ALL
        .iter()
        .zip(obs_blend_mode_params.iter().zip(c))
    {
        assert_eq!(as_oracle(*rust), *c, "{mode:?}");
    }
    let bytes = size_of_val(&obs_blend_mode_params);
    // SAFETY: both are tables of `MODE_COUNT` rows of five `int`s (layout
    // checked above), read as plain bytes; neither has padding.
    let (rust, c) = unsafe {
        (
            slice::from_raw_parts(obs_blend_mode_params.as_ptr().cast::<u8>(), bytes),
            slice::from_raw_parts(c.as_ptr().cast::<u8>(), bytes),
        )
    };
    assert_eq!(rust, c);
}

/// The safe core's lookup gives the C row for every mode.
#[test]
fn parity_blend_params_matches_c() {
    let c = oracle_table();
    for mode in BlendMode::ALL {
        let p = blend_params(mode);
        let row = c[mode as usize];
        assert_eq!(p.src_color as c_int, row.src_color, "{mode:?}");
        assert_eq!(p.src_alpha as c_int, row.src_alpha, "{mode:?}");
        assert_eq!(p.dst_color as c_int, row.dst_color, "{mode:?}");
        assert_eq!(p.dst_alpha as c_int, row.dst_alpha, "{mode:?}");
        assert_eq!(p.op as c_int, row.op, "{mode:?}");
    }
}
