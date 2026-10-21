//! Tier 1 mirror of `test/cmocka/test_scene_blend.c`.
//!
//! The six C tests keep their names (without the `test_` prefix) and their
//! cases. The edge cases after them are new. `test_darken_transparent`
//! still covers Darken drawn end to end.

use obs_scene::blend::{
    BlendMode, BlendOp, BlendParams, BlendType, MODE_COUNT, blend_mode_params, blend_params,
};

mod parity;

/// A row of `expected` in the C test: mode, name, the four factors
/// (`src_color`, `src_alpha`, `dst_color`, `dst_alpha`) and the operation.
type Expected = (
    BlendMode,
    &'static str,
    BlendType,
    BlendType,
    BlendType,
    BlendType,
    BlendOp,
);

/// `expected` in the C test.
const EXPECTED: [Expected; 7] = [
    (
        BlendMode::Normal,
        "normal",
        BlendType::One,
        BlendType::One,
        BlendType::InvSrcAlpha,
        BlendType::InvSrcAlpha,
        BlendOp::Add,
    ),
    (
        BlendMode::Additive,
        "additive",
        BlendType::One,
        BlendType::One,
        BlendType::One,
        BlendType::One,
        BlendOp::Add,
    ),
    (
        BlendMode::Subtract,
        "subtract",
        BlendType::One,
        BlendType::One,
        BlendType::One,
        BlendType::One,
        BlendOp::ReverseSubtract,
    ),
    (
        BlendMode::Screen,
        "screen",
        BlendType::One,
        BlendType::One,
        BlendType::InvSrcColor,
        BlendType::InvSrcAlpha,
        BlendOp::Add,
    ),
    (
        BlendMode::Multiply,
        "multiply",
        BlendType::DstColor,
        BlendType::DstAlpha,
        BlendType::InvSrcAlpha,
        BlendType::InvSrcAlpha,
        BlendOp::Add,
    ),
    (
        BlendMode::Lighten,
        "lighten",
        BlendType::One,
        BlendType::One,
        BlendType::One,
        BlendType::One,
        BlendOp::Max,
    ),
    (
        BlendMode::Darken,
        "darken",
        BlendType::One,
        BlendType::One,
        BlendType::One,
        BlendType::One,
        BlendOp::Min,
    ),
];

fn row(mode: BlendMode) -> BlendParams {
    blend_mode_params()[mode as usize]
}

/// `test_blend_modes_are_row_numbers`: the enum values are the row numbers.
#[test]
fn blend_modes_are_row_numbers() {
    assert_eq!(BlendMode::Normal as i32, 0);
    assert_eq!(BlendMode::Additive as i32, 1);
    assert_eq!(BlendMode::Subtract as i32, 2);
    assert_eq!(BlendMode::Screen as i32, 3);
    assert_eq!(BlendMode::Multiply as i32, 4);
    assert_eq!(BlendMode::Lighten as i32, 5);
    assert_eq!(BlendMode::Darken as i32, 6);
}

/// `test_one_row_per_mode`: one row per mode and no more. Characterized, not
/// endorsed: C indexes the table with the stored mode unchecked, so a mode
/// past Darken has no row. The safe API only takes a [`BlendMode`].
#[test]
fn one_row_per_mode() {
    assert_eq!(blend_mode_params().len(), BlendMode::Darken as usize + 1);
    assert_eq!(blend_mode_params().len(), EXPECTED.len());
    assert_eq!(MODE_COUNT, EXPECTED.len());
}

/// `test_every_mode_factors_and_op`.
#[test]
fn every_mode_factors_and_op() {
    for (i, &(mode, name, src_color, src_alpha, dst_color, dst_alpha, op)) in
        EXPECTED.iter().enumerate()
    {
        assert_eq!(mode as usize, i);
        let got = row(mode);
        assert_eq!(got.src_color, src_color, "blend mode {name}");
        assert_eq!(got.src_alpha, src_alpha, "blend mode {name}");
        assert_eq!(got.dst_color, dst_color, "blend mode {name}");
        assert_eq!(got.dst_alpha, dst_alpha, "blend mode {name}");
        assert_eq!(got.op, op, "blend mode {name}");
    }
}

/// `test_normal_is_premultiplied_over`: one, inverse source alpha, add.
#[test]
fn normal_is_premultiplied_over() {
    let normal = row(BlendMode::Normal);
    assert_eq!(normal.src_color, BlendType::One);
    assert_eq!(normal.dst_color, BlendType::InvSrcAlpha);
    assert_eq!(normal.op, BlendOp::Add);
}

/// `test_factor_one_modes_differ_by_op`. Characterized, not endorsed: alpha
/// goes through the same operation as color, so Subtract gives
/// `dst_a - src_a` and Darken `min(src_a, dst_a)`.
#[test]
fn factor_one_modes_differ_by_op() {
    let cases = [
        (BlendMode::Additive, BlendOp::Add),
        (BlendMode::Subtract, BlendOp::ReverseSubtract),
        (BlendMode::Lighten, BlendOp::Max),
        (BlendMode::Darken, BlendOp::Min),
    ];
    for (mode, op) in cases {
        let got = row(mode);
        assert_eq!(got.src_color, BlendType::One, "{mode:?}");
        assert_eq!(got.src_alpha, BlendType::One, "{mode:?}");
        assert_eq!(got.dst_color, BlendType::One, "{mode:?}");
        assert_eq!(got.dst_alpha, BlendType::One, "{mode:?}");
        assert_eq!(got.op, op, "{mode:?}");
    }
}

/// `test_screen_and_multiply_split_color_and_alpha`. Characterized, not
/// endorsed: Multiply's alpha is `dst_a`.
#[test]
fn screen_and_multiply_split_color_and_alpha() {
    let screen = row(BlendMode::Screen);
    let multiply = row(BlendMode::Multiply);
    assert_eq!(screen.dst_color, BlendType::InvSrcColor);
    assert_eq!(screen.dst_alpha, BlendType::InvSrcAlpha);
    assert_eq!(multiply.src_color, BlendType::DstColor);
    assert_eq!(multiply.src_alpha, BlendType::DstAlpha);
    assert_eq!(multiply.dst_alpha, BlendType::InvSrcAlpha);
}

// New edge cases.

/// [`blend_params`] is the table's row for every mode.
#[test]
fn blend_params_is_the_row() {
    for mode in BlendMode::ALL {
        assert_eq!(blend_params(mode), row(mode), "{mode:?}");
    }
}

/// `BlendMode::ALL` walks the rows in order.
#[test]
fn all_modes_in_row_order() {
    for (i, mode) in BlendMode::ALL.into_iter().enumerate() {
        assert_eq!(mode as usize, i);
    }
}

/// The factor and operation values are those of `graphics.h`, so the C ABI
/// can carry them as they are.
#[test]
fn graphics_enum_values_match_graphics_h() {
    let types = [
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
    for (i, t) in types.into_iter().enumerate() {
        assert_eq!(t as usize, i, "{t:?}");
    }
    let ops = [
        BlendOp::Add,
        BlendOp::Subtract,
        BlendOp::ReverseSubtract,
        BlendOp::Min,
        BlendOp::Max,
    ];
    for (i, op) in ops.into_iter().enumerate() {
        assert_eq!(op as usize, i, "{op:?}");
    }
}

/// No row uses `GS_BLEND_OP_SUBTRACT`, the zero factor or the saturated
/// source alpha; Subtract is the reverse operation (`dst - src`).
#[test]
fn unused_values_stay_unused() {
    for mode in BlendMode::ALL {
        let p = row(mode);
        assert_ne!(p.op, BlendOp::Subtract, "{mode:?}");
        for f in [p.src_color, p.src_alpha, p.dst_color, p.dst_alpha] {
            assert_ne!(f, BlendType::Zero, "{mode:?}");
            assert_ne!(f, BlendType::SrcAlphaSat, "{mode:?}");
        }
    }
}

/// Only Screen and Multiply draw color and alpha with different factors.
#[test]
fn only_screen_and_multiply_split() {
    for mode in BlendMode::ALL {
        let p = row(mode);
        let split = p.src_color != p.src_alpha || p.dst_color != p.dst_alpha;
        assert_eq!(
            split,
            matches!(mode, BlendMode::Screen | BlendMode::Multiply),
            "{mode:?}"
        );
    }
}
