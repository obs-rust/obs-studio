//! Safe core of `libobs/graphics/graphics-immediate.c.h`: the decisions of
//! the immediate-mode vertex builder of `graphics.c` (`gs_render_start`,
//! `gs_vertex*`, `gs_normal*`, `gs_color`, `gs_texcoord*`,
//! `gs_render_stop` and `gs_render_save`).
//!
//! - [`unit_ok`] (`gs_texcoord2v`): a texture unit can take a coordinate,
//!   asked before `graphics.c` indexes `texverts[unit]`: one of the
//!   [`TEX_ARRAYS`] arrays, and in immediate mode only array 0, the one the
//!   immediate buffer backs.
//! - [`push_ok`] (`validvertsize`): an array holding `num` vertices takes
//!   one more; in immediate mode none past [`IMMEDIATE_COUNT`].
//! - [`draw`] (`gs_render_stop`): the vertices to draw, the vertex count cut
//!   to each non-empty normal, colour or texture 0 count that differs, and
//!   which of them differ (each logs).
//! - [`save`] (`gs_render_save`): the buffer's vertex count, the leading
//!   non-empty texture arrays it takes, and the first array after them that
//!   `graphics.c` frees.
//!
//! `graphics.c` keeps every read of the arrays' counts, every `da_init`,
//! `da_free` and `da_push_back`, `gs_vbdata_create()` and
//! `gs_vbdata_destroy()`, the immediate buffer's pointer and capacity
//! stores, the vertex buffer calls, `gs_draw()` and every `blog()`. Nothing
//! here keeps state or calls libobs.
//!
//! Characterized, not endorsed (`test_graphics_immediate_steps`), and kept:
//! the stop looks at no other array when there is no vertex; a longer array
//! logs its error too and cuts nothing; an empty texture array 0 before a
//! filled one gives the buffer no texture array.
//!
//! Intentional differences from the C:
//! - [`unit_ok`] refuses a unit outside 0-15, which the C indexed
//!   `texverts` with, reading and writing past it; and units 1-15 in
//!   immediate mode, where the C pushed to a heap array that the immediate
//!   buffer never draws and the next start dropped without a free.
//! - [`save`] gives the buffer the smallest count of the vertex array and
//!   every other array it takes (the non-empty normal and colour arrays and
//!   its texture arrays), where the C gave it the vertex count, so the
//!   device read past a shorter array.
//! - [`save`] names the texture arrays after the ones the buffer takes,
//!   which `graphics.c` frees; the C dropped them without a free.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

/// `IMMEDIATE_COUNT` (`GS_IMMEDIATE_COUNT`): the vertices of the immediate
/// buffer.
pub const IMMEDIATE_COUNT: usize = 512;

/// `GS_IMMEDIATE_TEX_ARRAYS`: the texture arrays of the graphics context.
pub const TEX_ARRAYS: usize = 16;

/// What `gs_render_stop` draws (`struct gs_immediate_draw`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Draw {
    /// The vertices to draw; 0 when there is no vertex.
    pub num: usize,
    /// The normal array is not empty and differs from the vertex count.
    pub norms_mismatch: bool,
    /// The colour array is not empty and differs from the vertex count.
    pub colors_mismatch: bool,
    /// Texture array 0 is not empty and differs from the vertex count.
    pub tex_mismatch: bool,
}

/// The vertex buffer `gs_render_save` makes (`struct gs_immediate_save`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Save {
    /// The buffer's vertex count; 0 when there is no vertex, and no buffer.
    pub num: usize,
    /// The texture arrays the buffer takes: the leading non-empty ones.
    pub num_tex: usize,
    /// The first texture array the buffer leaves that `graphics.c` frees,
    /// up to the last; [`TEX_ARRAYS`] frees none.
    pub free_from: usize,
}

/// `gs_immediate_unit_ok()`: texture `unit` can take a coordinate.
#[must_use]
pub fn unit_ok(immediate: bool, unit: i32) -> bool {
    todo!("{immediate} {unit}")
}

/// `gs_immediate_push_ok()`: an array holding `num` vertices takes one
/// more.
#[must_use]
pub fn push_ok(immediate: bool, num: usize) -> bool {
    todo!("{immediate} {num}")
}

/// `gs_immediate_draw()`: what `gs_render_stop` draws, from the counts of
/// the vertex, normal, colour and texture 0 arrays.
#[must_use]
pub fn draw(verts: usize, norms: usize, colors: usize, tex0: usize) -> Draw {
    todo!("{verts} {norms} {colors} {tex0}")
}

/// `gs_immediate_save()`: the buffer `gs_render_save` makes, from the counts
/// of the vertex, normal and colour arrays and of the texture arrays.
#[must_use]
pub fn save(verts: usize, norms: usize, colors: usize, tex: &[usize; TEX_ARRAYS]) -> Save {
    todo!("{verts} {norms} {colors} {tex:?}")
}
