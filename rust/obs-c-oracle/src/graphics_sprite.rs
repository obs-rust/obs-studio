//! `oracle_*` build of `libobs/graphics/graphics-sprite.c.h`
//! (oracle/graphics_sprite.c): the sprite and texture upload geometry of
//! `graphics.c`, renamed. The structs are declared independently of the
//! port; the enums are read as `c_int`.

use core::ffi::c_int;

use super::graphics_math::OracleVec3;
use super::vec2::OracleVec2;

/// Index of each entry in `oracle_graphics_sprite_constant`, in the C's
/// order.
pub const CONSTANT_NAMES: [&str; 14] = [
    "GS_SPRITE_FLIP_U",
    "GS_SPRITE_FLIP_V",
    "GS_SPRITE_BUILD_MIPMAPS",
    "GS_SPRITE_RENDER_TARGET",
    "GS_SPRITE_QUAD_DRAW",
    "GS_SPRITE_QUAD_NOT_2D",
    "GS_SPRITE_QUAD_NO_SIZE",
    "GS_TEXTURE_UPLOAD_NONE",
    "GS_TEXTURE_UPLOAD_FLIPPED",
    "GS_TEXTURE_UPLOAD_WHOLE",
    "GS_TEXTURE_UPLOAD_ROWS",
    "GS_TEXTURE_MIP_OK",
    "GS_TEXTURE_MIP_NOT_POW2",
    "GS_TEXTURE_MIP_RENDER_TARGET",
];

/// Index of each entry in `oracle_graphics_sprite_layout`, in the C's order.
pub const LAYOUT_NAMES: [&str; 17] = [
    "sizeof(struct gs_sprite_uv)",
    "offsetof(gs_sprite_uv, start_u)",
    "offsetof(gs_sprite_uv, end_u)",
    "offsetof(gs_sprite_uv, start_v)",
    "offsetof(gs_sprite_uv, end_v)",
    "sizeof(struct gs_texture_upload_plan)",
    "offsetof(gs_texture_upload_plan, row_copy)",
    "offsetof(gs_texture_upload_plan, whole_size)",
    "offsetof(gs_texture_upload_plan, src_offset)",
    "offsetof(gs_texture_upload_plan, mode)",
    "sizeof(struct gs_texture_mip_plan)",
    "offsetof(gs_texture_mip_plan, flags)",
    "offsetof(gs_texture_mip_plan, levels)",
    "offsetof(gs_texture_mip_plan, warning)",
    "sizeof(enum gs_sprite_quad_refusal)",
    "sizeof(enum gs_texture_upload_mode)",
    "sizeof(enum gs_texture_mip_warning)",
];

/// `struct gs_sprite_uv`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct OracleSpriteUv {
    pub start_u: f32,
    pub end_u: f32,
    pub start_v: f32,
    pub end_v: f32,
}

/// `struct gs_texture_upload_plan`, the mode as `int`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleUploadPlan {
    pub row_copy: usize,
    pub whole_size: usize,
    pub src_offset: usize,
    pub mode: c_int,
}

/// `struct gs_texture_mip_plan`, the warning as `int`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleMipPlan {
    pub flags: u32,
    pub levels: u32,
    pub warning: c_int,
}

unsafe extern "C" {
    /// `void gs_sprite_build(struct vec3 *, struct vec2 *, float, float,
    /// struct gs_sprite_uv)`.
    pub fn oracle_gs_sprite_build(
        points: *mut OracleVec3,
        uvs: *mut OracleVec2,
        cx: f32,
        cy: f32,
        uv: OracleSpriteUv,
    );
    pub fn oracle_gs_sprite_uv_norm(flip: u32) -> OracleSpriteUv;
    pub fn oracle_gs_sprite_uv_rect(width: u32, height: u32, flip: u32) -> OracleSpriteUv;
    pub fn oracle_gs_sprite_uv_subregion(
        x: u32,
        y: u32,
        cx: u32,
        cy: u32,
        tex_cx: u32,
        tex_cy: u32,
        flip: u32,
    ) -> OracleSpriteUv;
    pub fn oracle_gs_sprite_subregion_is_whole(
        x: u32,
        y: u32,
        cx: u32,
        cy: u32,
        tex_cx: u32,
        tex_cy: u32,
    ) -> bool;
    pub fn oracle_gs_sprite_quad_refusal(
        has_texture: bool,
        texture_2d: bool,
        width: f32,
        height: f32,
    ) -> c_int;
    pub fn oracle_gs_sprite_quad_texture_size(size: f32) -> bool;
    pub fn oracle_gs_sprite_quad_flipped_buffer(flip: u32) -> bool;
    pub fn oracle_gs_texture_upload_plan(
        linesize: u32,
        linesize_out: u32,
        height: usize,
        flip: bool,
    ) -> OracleUploadPlan;
    pub fn oracle_gs_texture_is_pow2(size: u32) -> bool;
    pub fn oracle_gs_texture_mip_plan(
        width: u32,
        height: u32,
        flags: u32,
        levels: u32,
    ) -> OracleMipPlan;
    /// The entry named `CONSTANT_NAMES[which]`; -1 out of range.
    pub fn oracle_graphics_sprite_constant(which: usize) -> i64;
    /// The entry named `LAYOUT_NAMES[which]`; 0 out of range.
    pub fn oracle_graphics_sprite_layout(which: usize) -> usize;
}
