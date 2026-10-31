//! Safe core of `libobs/graphics/graphics-immediate.c.h`: the decisions of
//! the immediate-mode vertex builder of `graphics.c` (`gs_render_start`,
//! `gs_vertex*`, `gs_normal*`, `gs_color`, `gs_texcoord*`,
//! `gs_render_stop` and `gs_render_save`). One function per decision, each
//! asked where the C made it, from the counts `graphics.c` reads there:
//!
//! - [`unit_ok`] (`gs_texcoord2v`): a texture unit can take a coordinate,
//!   asked before `graphics.c` indexes `texverts[unit]`: one of the
//!   [`TEX_ARRAYS`] arrays, and in immediate mode only array 0, the one the
//!   immediate buffer backs.
//! - [`push_ok`] (`validvertsize`): an array holding `num` vertices takes
//!   one more; in immediate mode none past [`IMMEDIATE_COUNT`].
//! - [`mismatch`] and [`cut`] (`gs_render_stop`, for each of the normal,
//!   colour and texture 0 arrays): a non-empty array whose count differs from
//!   the vertices (it logs), then the draw cut to its count, read again
//!   after the log.
//! - [`save_cut`] (`gs_render_save`, for the normal, colour and each taken
//!   texture array): the buffer's vertex count cut to a shorter non-empty
//!   array.
//! - [`save_free_from`] (`gs_render_save`): the first texture array the
//!   buffer leaves, which `graphics.c` frees.
//!
//! `graphics.c` keeps every read of the arrays' counts, the no-vertex tests,
//! the loop that counts the leading non-empty texture arrays, every
//! `da_init`, `da_free` and `da_push_back`, `gs_vbdata_create()` and
//! `gs_vbdata_destroy()`, the immediate buffer's pointer and capacity
//! stores, the vertex buffer calls, `gs_draw()` and every `blog()`. Nothing
//! here keeps state or calls libobs.
//!
//! Characterized, not endorsed (`test_graphics_immediate_steps`), and kept:
//! a longer array logs its error too and cuts nothing.
//!
//! Intentional differences from the C:
//! - [`unit_ok`] refuses a unit outside 0-15, which the C indexed
//!   `texverts` with, reading and writing past it; and units 1-15 in
//!   immediate mode, where the C pushed to a heap array that the immediate
//!   buffer never draws and the next start dropped without a free.
//! - [`save_cut`] cuts the buffer's vertex count to a shorter non-empty
//!   normal, colour or taken texture array, where the C kept the vertex
//!   count, so the device read past the shorter array.
//! - [`save_free_from`] names the texture arrays from the first one the
//!   buffer does not take, which `graphics.c` frees; the C freed none and
//!   dropped the ones after the first empty array without a free.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable,
    clippy::todo
)]

/// `IMMEDIATE_COUNT` (`GS_IMMEDIATE_COUNT`): the vertices of the immediate
/// buffer.
pub const IMMEDIATE_COUNT: usize = 512;

/// `GS_IMMEDIATE_TEX_ARRAYS`: the texture arrays of the graphics context.
pub const TEX_ARRAYS: usize = 16;

/// `gs_immediate_cut()`: the draw of `num` vertices, cut to a differing
/// array of `count`.
#[must_use]
pub fn cut(num: usize, count: usize) -> usize {
    num.min(count)
}

/// `gs_immediate_mismatch()`: an array of `count` differs from the `verts`
/// vertices; an empty one never does.
#[must_use]
pub fn mismatch(count: usize, verts: usize) -> bool {
    count != 0 && count != verts
}

/// `gs_immediate_push_ok()`: an array holding `num` vertices takes one
/// more.
#[must_use]
pub fn push_ok(immediate: bool, num: usize) -> bool {
    !(immediate && num == IMMEDIATE_COUNT)
}

/// `gs_immediate_save_cut()`: the buffer's vertex count of `num`, with an
/// array of `count` that the buffer takes: cut to a shorter non-empty one.
#[must_use]
pub fn save_cut(num: usize, count: usize) -> usize {
    if count == 0 { num } else { num.min(count) }
}

/// `gs_immediate_save_free_from()`: the first texture array that a buffer
/// of `num_tex` texture arrays leaves, freed up to the last.
#[must_use]
pub fn save_free_from(num_tex: usize) -> usize {
    num_tex.min(TEX_ARRAYS)
}

/// `gs_immediate_unit_ok()`: texture `unit` can take a coordinate.
#[must_use]
pub fn unit_ok(immediate: bool, unit: i32) -> bool {
    match usize::try_from(unit) {
        Ok(unit) if immediate => unit == 0,
        Ok(unit) => unit < TEX_ARRAYS,
        Err(_) => false,
    }
}
