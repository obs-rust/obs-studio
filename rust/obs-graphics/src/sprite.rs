//! Safe core of `libobs/graphics/graphics-sprite.c.h`: the sprite and
//! texture upload geometry of `graphics.c`.
//!
//! - [`sprite`] (`build_sprite`): the four points of a `cx` by `cy` sprite,
//!   `z` and the padding lane `w` 0, and their texture coordinates in
//!   triangle-strip order.
//! - [`sprite_uv`] (`build_sprite_norm`, the static sprite buffers),
//!   [`rect_uv`] (`build_sprite_rect`) and [`subsprite_uv`]
//!   (`build_subsprite_norm`): the coordinates, 0 to 1, in texels, or of a
//!   subregion, each axis swapped by its own flip bit; other bits are
//!   ignored. A size converts to `f32` as C's `(float)uint32_t` does,
//!   rounded to the nearest (ties to even); the subregion's `x + cx` is an
//!   `f32` sum and each coordinate one `f32` division.
//! - [`subregion_is_whole`] (`gs_draw_sprite_subregion`): the shortcut to
//!   a plain sprite for `(0, 0)` and the texture's size.
//! - [`quad_refusal`], [`quad_texture_size`] and [`quad_flipped_buffer`]
//!   (`gs_draw_quadf`): a texture must be 2D; without one neither size may
//!   be 0; an axis of size 0 (or -0.0) takes the texture's; any flip bit
//!   draws the static buffer flipped on v.
//! - [`upload_plan`] (`gs_texture_set_image`): the bytes per row (the
//!   shorter line), and a flipped copy from the last row, one copy of the
//!   whole image for equal line sizes, or row by row. `usize` arithmetic
//!   wraps, as C's `size_t` does.
//! - [`is_pow2`] and [`texture_flags`] (`gs_texture_create`): the mipmap
//!   rules.
//!
//! `graphics.c` keeps the device calls, the vertex buffers, the matrix
//! stack, `gs_texture_map()`/`gs_texture_unmap()`, every `memcpy` and the
//! logs. Nothing here keeps state or calls libobs.
//!
//! Characterized, not endorsed (`test_sprite_geometry_steps`), and kept: a
//! subregion of a 0-size texture divides by 0 (`+inf`, and NaN for `0 /
//! 0`); nothing clamps a subregion to the texture; any flip bit, `GS_FLIP_U`
//! alone included, draws a plain texture with the buffer flipped on v; a
//! NaN or negative size draws; a 1x1 texture warns that it is not a power
//! of two.
//!
//! One change from the C before the move, made in the move: a flip with a
//! height of 0 is [`UploadMode::None`] before the start row is computed
//! (that was `(height - 1) * linesize`, a wrapped offset the C moved its
//! source pointer by, undefined, before its loop found no row). No
//! intentional differences from `graphics-sprite.c.h`.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

use crate::vec2::Vec2;
use crate::vec3::Vec3;

/// `GS_FLIP_U` (`graphics.h`): flip the sprite on u.
pub const FLIP_U: u32 = 1 << 0;
/// `GS_FLIP_V` (`graphics.h`): flip the sprite on v.
pub const FLIP_V: u32 = 1 << 1;
/// `GS_BUILD_MIPMAPS` (`graphics.h`).
pub const BUILD_MIPMAPS: u32 = 1 << 0;
/// `GS_RENDER_TARGET` (`graphics.h`).
pub const RENDER_TARGET: u32 = 1 << 2;

/// The texture coordinates of a sprite's edges (`struct gs_sprite_uv`): u
/// from left to right and v from top to bottom, before a flip swaps them.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SpriteUv {
    pub start_u: f32,
    pub end_u: f32,
    pub start_v: f32,
    pub end_v: f32,
}

/// `build_sprite`: points `(0, 0)`, `(cx, 0)`, `(0, cy)` and `(cx, cy)`
/// with `z` and `w` 0 (`vec3_zero`, `vec3_set`), and texture coordinates
/// `(start_u, start_v)`, `(end_u, start_v)`, `(start_u, end_v)` and
/// `(end_u, end_v)`. The floats are copied, never computed on.
#[must_use]
pub const fn sprite(cx: f32, cy: f32, uv: SpriteUv) -> ([Vec3; 4], [Vec2; 4]) {
    (
        [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(cx, 0.0, 0.0),
            Vec3::new(0.0, cy, 0.0),
            Vec3::new(cx, cy, 0.0),
        ],
        [
            Vec2::new(uv.start_u, uv.start_v),
            Vec2::new(uv.end_u, uv.start_v),
            Vec2::new(uv.start_u, uv.end_v),
            Vec2::new(uv.end_u, uv.end_v),
        ],
    )
}

/// `assign_sprite_uv` / `assign_sprite_rect`: 0 to `size`, or `size` to 0
/// when flipped.
const fn span(size: f32, flip: bool) -> (f32, f32) {
    if flip { (size, 0.0) } else { (0.0, size) }
}

const fn flips_u(flip: u32) -> bool {
    flip & FLIP_U != 0
}

const fn flips_v(flip: u32) -> bool {
    flip & FLIP_V != 0
}

/// `build_sprite_norm`'s coordinates: 0 to 1 on each axis, swapped by
/// [`FLIP_U`] and [`FLIP_V`]; other bits are ignored.
#[must_use]
pub const fn sprite_uv(flip: u32) -> SpriteUv {
    let (start_u, end_u) = span(1.0, flips_u(flip));
    let (start_v, end_v) = span(1.0, flips_v(flip));
    SpriteUv {
        start_u,
        end_u,
        start_v,
        end_v,
    }
}

/// `(float)size`: the nearest `f32`, ties to even, as C converts.
#[allow(clippy::cast_precision_loss)] // the C's own rounding
#[allow(clippy::as_conversions)] // no lossless `From` exists; `as` rounds as C does
const fn to_f32(size: u32) -> f32 {
    size as f32
}

/// `build_sprite_rect`'s coordinates: 0 to the texture's size in texels.
#[must_use]
pub const fn rect_uv(width: u32, height: u32, flip: u32) -> SpriteUv {
    let (start_u, end_u) = span(to_f32(width), flips_u(flip));
    let (start_v, end_v) = span(to_f32(height), flips_v(flip));
    SpriteUv {
        start_u,
        end_u,
        start_v,
        end_v,
    }
}

/// `build_subsprite_norm` on one axis: `pos / size` and `(pos + len) /
/// size`, swapped when flipped. Plain IEEE `f32`: a size of 0 gives `+inf`,
/// or NaN for `0 / 0`.
#[allow(clippy::arithmetic_side_effects)] // f32: no overflow panic, IEEE as the C
fn sub_span(pos: u32, len: u32, size: u32, flip: bool) -> (f32, f32) {
    let pos = to_f32(pos);
    let len = to_f32(len);
    let size = to_f32(size);
    if flip {
        ((pos + len) / size, pos / size)
    } else {
        (pos / size, (pos + len) / size)
    }
}

/// `build_subsprite_norm`'s coordinates of the subregion `(x, y, cx, cy)`
/// of a `tex_cx` by `tex_cy` texture.
#[must_use]
pub fn subsprite_uv(
    x: u32,
    y: u32,
    cx: u32,
    cy: u32,
    tex_cx: u32,
    tex_cy: u32,
    flip: u32,
) -> SpriteUv {
    let (start_u, end_u) = sub_span(x, cx, tex_cx, flips_u(flip));
    let (start_v, end_v) = sub_span(y, cy, tex_cy, flips_v(flip));
    SpriteUv {
        start_u,
        end_u,
        start_v,
        end_v,
    }
}

/// `gs_draw_sprite_subregion`: the region is the whole texture, so it is
/// drawn as a plain sprite.
#[must_use]
pub const fn subregion_is_whole(
    x: u32,
    y: u32,
    cx: u32,
    cy: u32,
    tex_cx: u32,
    tex_cy: u32,
) -> bool {
    x == 0 && y == 0 && cx == tex_cx && cy == tex_cy
}

/// Why `gs_draw_quadf` draws nothing (`enum gs_sprite_quad_refusal`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuadRefusal {
    /// It draws.
    Draw = 0,
    /// "A sprite must be a 2D texture".
    NotTwoD = 1,
    /// "A sprite cannot be drawn without a width/height".
    NoSize = 2,
}

/// `gs_draw_quadf`'s refusals: with a texture it must be 2D
/// (`texture_2d` is read only then); without one neither size may be 0
/// (-0.0 is 0, NaN is not).
#[must_use]
pub fn quad_refusal(has_texture: bool, texture_2d: bool, width: f32, height: f32) -> QuadRefusal {
    if has_texture {
        if texture_2d {
            QuadRefusal::Draw
        } else {
            QuadRefusal::NotTwoD
        }
    } else if quad_texture_size(width) || quad_texture_size(height) {
        QuadRefusal::NoSize
    } else {
        QuadRefusal::Draw
    }
}

/// `gs_draw_quadf` takes this axis from the texture: `size == 0.0f`, so 0
/// and -0.0.
#[must_use]
#[allow(clippy::float_cmp)] // the C's exact compare
pub fn quad_texture_size(size: f32) -> bool {
    size == 0.0
}

/// `gs_draw_quadf` draws a texture that is not a rectangle texture with the
/// static buffer flipped on v: any bit set.
#[must_use]
pub const fn quad_flipped_buffer(flip: u32) -> bool {
    flip != 0
}

/// How `gs_texture_set_image` copies (`enum gs_texture_upload_mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadMode {
    /// Nothing: a flip with a height of 0.
    None = 0,
    /// Row by row from the source's last row up.
    Flipped = 1,
    /// One copy of [`UploadPlan::whole_size`] bytes.
    Whole = 2,
    /// Row by row, top down.
    Rows = 3,
}

/// `struct gs_texture_upload_plan`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UploadPlan {
    /// The shorter line size: the bytes each row copies.
    pub row_copy: usize,
    /// [`UploadMode::Whole`]: `row_copy * height`, wrapping.
    pub whole_size: usize,
    /// [`UploadMode::Flipped`]: `(height - 1) * linesize`, the last row.
    pub src_offset: usize,
    pub mode: UploadMode,
}

/// `u32` to `usize`: lossless where libobs builds (`usize` is 32 or 64
/// bits), as C's conversion to `size_t`.
#[allow(clippy::as_conversions)] // `usize: From<u32>` does not exist
const fn to_usize(v: u32) -> usize {
    v as usize
}

/// `gs_texture_set_image`'s copy plan for `height` rows of `linesize`
/// bytes into the mapped rows of `linesize_out` bytes.
#[must_use]
pub const fn upload_plan(
    linesize: u32,
    linesize_out: u32,
    height: usize,
    flip: bool,
) -> UploadPlan {
    let row_copy = to_usize(if linesize < linesize_out {
        linesize
    } else {
        linesize_out
    });
    let mut plan = UploadPlan {
        row_copy,
        whole_size: 0,
        src_offset: 0,
        mode: UploadMode::Rows,
    };
    if flip {
        // A flip with no rows copies nothing; the C before the move
        // computed (height - 1) * linesize first, which wrapped.
        if height == 0 {
            plan.mode = UploadMode::None;
            return plan;
        }
        plan.src_offset = height.wrapping_sub(1).wrapping_mul(to_usize(linesize));
        plan.mode = UploadMode::Flipped;
    } else if linesize == linesize_out {
        plan.whole_size = row_copy.wrapping_mul(height);
        plan.mode = UploadMode::Whole;
    }
    plan
}

/// `is_pow2`: at least 2 and one bit set.
#[must_use]
pub const fn is_pow2(size: u32) -> bool {
    size >= 2 && size.is_power_of_two()
}

/// The warning `gs_texture_create` logs (`enum gs_texture_mip_warning`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MipWarning {
    /// No warning; the flags and levels as given.
    Ok = 0,
    /// "Cannot use mipmaps with a non-power-of-two texture."
    NotPow2 = 1,
    /// "Cannot use mipmaps with render targets."
    RenderTarget = 2,
}

/// `struct gs_texture_mip_plan`: what `gs_texture_create` passes to the
/// device, and its warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MipPlan {
    pub flags: u32,
    pub levels: u32,
    pub warning: MipWarning,
}

/// `gs_texture_create`'s mipmap rules: mipmaps are on with
/// [`BUILD_MIPMAPS`] or `levels` other than 1. Then a width or height that
/// is not [`is_pow2`] warns and turns them off ([`BUILD_MIPMAPS`] cleared,
/// 1 level); else [`RENDER_TARGET`] does the same with its own warning.
/// Every other flag is kept.
#[must_use]
pub const fn texture_flags(width: u32, height: u32, flags: u32, levels: u32) -> MipPlan {
    let pow2tex = is_pow2(width) && is_pow2(height);
    let uses_mipmaps = flags & BUILD_MIPMAPS != 0 || levels != 1;
    let warning = if uses_mipmaps && !pow2tex {
        MipWarning::NotPow2
    } else if uses_mipmaps && flags & RENDER_TARGET != 0 {
        MipWarning::RenderTarget
    } else {
        return MipPlan {
            flags,
            levels,
            warning: MipWarning::Ok,
        };
    };
    MipPlan {
        flags: flags & !BUILD_MIPMAPS,
        levels: 1,
        warning,
    }
}
