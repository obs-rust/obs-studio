//! C ABI for `libobs/graphics/graphics-immediate.c.h`, the decisions of the
//! immediate-mode vertex builder of `graphics.c`, declared in
//! `libobs/graphics/graphics-immediate.h` without `EXPORT`.
//!
//! `graphics.c` (still C) calls these where it made each decision, with the
//! counts it read there: `gs_texcoord2v()` asks [`gs_immediate_unit_ok`]
//! before it indexes `texverts[unit]`, the four push gates ask
//! [`gs_immediate_push_ok`] (`validvertsize()`), `gs_render_stop()` asks
//! [`gs_immediate_mismatch`] and, after its log, [`gs_immediate_cut`] for
//! each of the normal, colour and texture 0 arrays, and `gs_render_save()`
//! asks [`gs_immediate_save_cut`] for each array its buffer takes and
//! [`gs_immediate_save_free_from`]. It keeps the arrays, every read, push
//! and free, the vertex buffer calls and the logs. Everything crosses by
//! value. The shims take no lock, call nothing in libobs and stay hidden
//! (`rust-hidden.txt`).
//!
//! Intentional differences from C: those of [`crate::immediate`].

use core::ffi::c_int;

use crate::immediate as core_immediate;

/// `size_t gs_immediate_cut(size_t num, size_t count)`.
#[unsafe(no_mangle)]
pub extern "C" fn gs_immediate_cut(num: usize, count: usize) -> usize {
    core_immediate::cut(num, count)
}

/// `bool gs_immediate_mismatch(size_t count, size_t verts)`.
#[unsafe(no_mangle)]
pub extern "C" fn gs_immediate_mismatch(count: usize, verts: usize) -> bool {
    core_immediate::mismatch(count, verts)
}

/// `bool gs_immediate_push_ok(bool immediate, size_t num)`.
#[unsafe(no_mangle)]
pub extern "C" fn gs_immediate_push_ok(immediate: bool, num: usize) -> bool {
    core_immediate::push_ok(immediate, num)
}

/// `size_t gs_immediate_save_cut(size_t num, size_t count)`.
#[unsafe(no_mangle)]
pub extern "C" fn gs_immediate_save_cut(num: usize, count: usize) -> usize {
    core_immediate::save_cut(num, count)
}

/// `size_t gs_immediate_save_free_from(size_t num_tex)`.
#[unsafe(no_mangle)]
pub extern "C" fn gs_immediate_save_free_from(num_tex: usize) -> usize {
    core_immediate::save_free_from(num_tex)
}

/// `bool gs_immediate_unit_ok(bool immediate, int unit)`.
#[unsafe(no_mangle)]
pub extern "C" fn gs_immediate_unit_ok(immediate: bool, unit: c_int) -> bool {
    core_immediate::unit_ok(immediate, unit)
}
