//! C ABI for `libobs/graphics/graphics-sprite.c.h`, the sprite and texture
//! upload geometry of `graphics.c`, declared in
//! `libobs/graphics/graphics-sprite.h` without `EXPORT`.
//!
//! `graphics.c` (still C) calls these where it made each decision, with the
//! values it read there: `gs_draw_quadf()`, `gs_draw_sprite_subregion()`,
//! `gs_texture_set_image()`, `gs_texture_create()`,
//! `gs_cubetexture_create()` ([`gs_texture_is_pow2`] only) and
//! `graphics_init_sprite_vbs()`. It keeps the device calls, the vertex
//! buffers (it passes [`gs_sprite_build`] the points and texture coordinate
//! arrays of a `gs_vb_data`), the matrix stack, the texture map, every
//! `memcpy` and the logs. Everything else crosses by value: `struct
//! gs_sprite_uv`, `struct gs_texture_upload_plan` and `struct
//! gs_texture_mip_plan` are mirrored, and the three enums cross as `int`
//! (`graphics.c` checks their size). The shims take no lock, call nothing in
//! libobs and stay hidden (`rust-hidden.txt`).
//!
//! Intentional difference from C: [`gs_sprite_build`] writes nothing
//! through a null `points` or `uvs` (the C writes through it); libobs always
//! passes a vertex buffer's arrays.

use core::ffi::c_int;
use core::ptr;

use super::vec2::vec2;
use super::vec3::vec3;
use crate::sprite::{self as core_sprite, SpriteUv};

/// `struct gs_sprite_uv` (`graphics-sprite.h`): four floats, 16 bytes.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[allow(non_camel_case_types)]
pub struct gs_sprite_uv {
    pub start_u: f32,
    pub end_u: f32,
    pub start_v: f32,
    pub end_v: f32,
}

impl From<SpriteUv> for gs_sprite_uv {
    fn from(uv: SpriteUv) -> Self {
        Self {
            start_u: uv.start_u,
            end_u: uv.end_u,
            start_v: uv.start_v,
            end_v: uv.end_v,
        }
    }
}

impl From<gs_sprite_uv> for SpriteUv {
    fn from(uv: gs_sprite_uv) -> Self {
        Self {
            start_u: uv.start_u,
            end_u: uv.end_u,
            start_v: uv.start_v,
            end_v: uv.end_v,
        }
    }
}

/// `struct gs_texture_upload_plan` (`graphics-sprite.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct gs_texture_upload_plan {
    pub row_copy: usize,
    pub whole_size: usize,
    pub src_offset: usize,
    /// `enum gs_texture_upload_mode`.
    pub mode: c_int,
}

/// `struct gs_texture_mip_plan` (`graphics-sprite.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct gs_texture_mip_plan {
    pub flags: u32,
    pub levels: u32,
    /// `enum gs_texture_mip_warning`.
    pub warning: c_int,
}

/// `void gs_sprite_build(struct vec3 *points, struct vec2 *uvs, float cx,
/// float cy, struct gs_sprite_uv uv)`: writes `points[0..3]` (every lane,
/// `z` and `w` 0) and `uvs[0..3]`.
///
/// # Safety
///
/// `points`, when not null, is valid for writes of four aligned `struct
/// vec3`, and `uvs`, when not null, of four `struct vec2`; neither is read.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_sprite_build(
    points: *mut vec3,
    uvs: *mut vec2,
    cx: f32,
    cy: f32,
    uv: gs_sprite_uv,
) {
    let (sprite_points, sprite_uvs) = core_sprite::sprite(cx, cy, uv.into());
    if !points.is_null() {
        let points_out: [vec3; 4] = sprite_points.map(vec3::from);
        // SAFETY: four writable, aligned vec3 (caller contract).
        unsafe { ptr::write(points.cast::<[vec3; 4]>(), points_out) };
    }
    if !uvs.is_null() {
        let uvs_out: [vec2; 4] = sprite_uvs.map(vec2::from);
        // SAFETY: four writable vec2 (caller contract).
        unsafe { ptr::write(uvs.cast::<[vec2; 4]>(), uvs_out) };
    }
}

/// `struct gs_sprite_uv gs_sprite_uv_norm(uint32_t flip)`.
#[unsafe(no_mangle)]
pub extern "C" fn gs_sprite_uv_norm(flip: u32) -> gs_sprite_uv {
    core_sprite::sprite_uv(flip).into()
}

/// `struct gs_sprite_uv gs_sprite_uv_rect(uint32_t width, uint32_t height,
/// uint32_t flip)`.
#[unsafe(no_mangle)]
pub extern "C" fn gs_sprite_uv_rect(width: u32, height: u32, flip: u32) -> gs_sprite_uv {
    core_sprite::rect_uv(width, height, flip).into()
}

/// `struct gs_sprite_uv gs_sprite_uv_subregion(uint32_t x, uint32_t y,
/// uint32_t cx, uint32_t cy, uint32_t tex_cx, uint32_t tex_cy, uint32_t
/// flip)`.
#[unsafe(no_mangle)]
pub extern "C" fn gs_sprite_uv_subregion(
    x: u32,
    y: u32,
    cx: u32,
    cy: u32,
    tex_cx: u32,
    tex_cy: u32,
    flip: u32,
) -> gs_sprite_uv {
    core_sprite::subsprite_uv(x, y, cx, cy, tex_cx, tex_cy, flip).into()
}

/// `bool gs_sprite_subregion_is_whole(uint32_t x, uint32_t y, uint32_t cx,
/// uint32_t cy, uint32_t tex_cx, uint32_t tex_cy)`.
#[unsafe(no_mangle)]
pub extern "C" fn gs_sprite_subregion_is_whole(
    x: u32,
    y: u32,
    cx: u32,
    cy: u32,
    tex_cx: u32,
    tex_cy: u32,
) -> bool {
    core_sprite::subregion_is_whole(x, y, cx, cy, tex_cx, tex_cy)
}

/// `enum gs_sprite_quad_refusal gs_sprite_quad_refusal(bool has_texture,
/// bool texture_2d, float width, float height)`.
#[unsafe(no_mangle)]
pub extern "C" fn gs_sprite_quad_refusal(
    has_texture: bool,
    texture_2d: bool,
    width: f32,
    height: f32,
) -> c_int {
    core_sprite::quad_refusal(has_texture, texture_2d, width, height) as c_int
}

/// `bool gs_sprite_quad_texture_size(float size)`.
#[unsafe(no_mangle)]
pub extern "C" fn gs_sprite_quad_texture_size(size: f32) -> bool {
    core_sprite::quad_texture_size(size)
}

/// `bool gs_sprite_quad_flipped_buffer(uint32_t flip)`.
#[unsafe(no_mangle)]
pub extern "C" fn gs_sprite_quad_flipped_buffer(flip: u32) -> bool {
    core_sprite::quad_flipped_buffer(flip)
}

/// `struct gs_texture_upload_plan gs_texture_upload_plan(uint32_t linesize,
/// uint32_t linesize_out, size_t height, bool flip)`.
#[unsafe(no_mangle)]
pub extern "C" fn gs_texture_upload_plan(
    linesize: u32,
    linesize_out: u32,
    height: usize,
    flip: bool,
) -> gs_texture_upload_plan {
    let plan = core_sprite::upload_plan(linesize, linesize_out, height, flip);
    gs_texture_upload_plan {
        row_copy: plan.row_copy,
        whole_size: plan.whole_size,
        src_offset: plan.src_offset,
        mode: plan.mode as c_int,
    }
}

/// `bool gs_texture_is_pow2(uint32_t size)`.
#[unsafe(no_mangle)]
pub extern "C" fn gs_texture_is_pow2(size: u32) -> bool {
    core_sprite::is_pow2(size)
}

/// `struct gs_texture_mip_plan gs_texture_mip_plan(uint32_t width, uint32_t
/// height, uint32_t flags, uint32_t levels)`.
#[unsafe(no_mangle)]
pub extern "C" fn gs_texture_mip_plan(
    width: u32,
    height: u32,
    flags: u32,
    levels: u32,
) -> gs_texture_mip_plan {
    let plan = core_sprite::texture_flags(width, height, flags, levels);
    gs_texture_mip_plan {
        flags: plan.flags,
        levels: plan.levels,
        warning: plan.warning as c_int,
    }
}
