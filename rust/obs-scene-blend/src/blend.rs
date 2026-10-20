//! Safe core of `obs_blend_mode_params`, the table `render_item_texture()`
//! in `obs-scene.c` reads to set the blend state of a scene item.

/// `enum gs_blend_type` in `libobs/graphics/graphics.h`, same numeric order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum BlendType {
    Zero = 0,
    One = 1,
    SrcColor = 2,
    InvSrcColor = 3,
    SrcAlpha = 4,
    InvSrcAlpha = 5,
    DstColor = 6,
    InvDstColor = 7,
    DstAlpha = 8,
    InvDstAlpha = 9,
    SrcAlphaSat = 10,
}

/// `enum gs_blend_op_type` in `libobs/graphics/graphics.h`, same numeric order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum BlendOp {
    Add = 0,
    Subtract = 1,
    ReverseSubtract = 2,
    Min = 3,
    Max = 4,
}

/// `enum obs_blending_type` in `libobs/obs.h`, same numeric order. The value
/// is the row of [`blend_mode_params`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum BlendMode {
    Normal = 0,
    Additive = 1,
    Subtract = 2,
    Screen = 3,
    Multiply = 4,
    Lighten = 5,
    Darken = 6,
}

impl BlendMode {
    /// Every mode, in row order.
    pub const ALL: [Self; MODE_COUNT] = [
        Self::Normal,
        Self::Additive,
        Self::Subtract,
        Self::Screen,
        Self::Multiply,
        Self::Lighten,
        Self::Darken,
    ];
}

/// Rows in the table: one per [`BlendMode`].
pub const MODE_COUNT: usize = 7;

/// One row: the factors `gs_blend_function_separate()` gets and the
/// operation `gs_blend_op()` gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlendParams {
    pub src_color: BlendType,
    pub src_alpha: BlendType,
    pub dst_color: BlendType,
    pub dst_alpha: BlendType,
    pub op: BlendOp,
}

const fn row(
    src_color: BlendType,
    src_alpha: BlendType,
    dst_color: BlendType,
    dst_alpha: BlendType,
    op: BlendOp,
) -> BlendParams {
    BlendParams {
        src_color,
        src_alpha,
        dst_color,
        dst_alpha,
        op,
    }
}

const TABLE: [BlendParams; MODE_COUNT] = {
    use BlendType::{DstAlpha, DstColor, InvSrcAlpha, InvSrcColor, One};
    [
        // Normal: premultiplied-alpha "over".
        row(One, One, InvSrcAlpha, InvSrcAlpha, BlendOp::Add),
        // Additive
        row(One, One, One, One, BlendOp::Add),
        // Subtract: dst - src, alpha too.
        row(One, One, One, One, BlendOp::ReverseSubtract),
        // Screen
        row(One, One, InvSrcColor, InvSrcAlpha, BlendOp::Add),
        // Multiply: the alpha comes out as dst_a.
        row(DstColor, DstAlpha, InvSrcAlpha, InvSrcAlpha, BlendOp::Add),
        // Lighten: max ignores the factors.
        row(One, One, One, One, BlendOp::Max),
        // Darken: min ignores the factors.
        row(One, One, One, One, BlendOp::Min),
    ]
};

/// The whole table, indexed by [`BlendMode`] as `usize`.
#[must_use]
pub const fn blend_mode_params() -> [BlendParams; MODE_COUNT] {
    TABLE
}

/// The row for `mode`.
#[must_use]
pub const fn blend_params(mode: BlendMode) -> BlendParams {
    TABLE[mode as usize]
}
