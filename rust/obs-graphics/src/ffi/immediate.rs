//! C ABI for `libobs/graphics/graphics-immediate.c.h`, the decisions of the
//! immediate-mode vertex builder of `graphics.c`, declared in
//! `libobs/graphics/graphics-immediate.h` without `EXPORT`.
//!
//! `graphics.c` (still C) calls these where it made each decision, with the
//! counts it read there: `gs_texcoord2v()` asks [`gs_immediate_unit_ok`]
//! before it indexes `texverts[unit]`, the four push gates ask
//! [`gs_immediate_push_ok`] (`validvertsize()`), `gs_render_stop()` asks
//! [`gs_immediate_draw`] and `gs_render_save()` asks [`gs_immediate_save`].
//! It keeps the arrays, every push and free, the vertex buffer calls and the
//! logs. `struct gs_immediate_draw` and `struct gs_immediate_save` are
//! mirrored and cross by value; the save's texture counts cross as a pointer
//! to `GS_IMMEDIATE_TEX_ARRAYS` of them. The shims take no lock, call
//! nothing in libobs and stay hidden (`rust-hidden.txt`).
//!
//! Intentional differences from C: those of [`crate::immediate`]. And
//! [`gs_immediate_save`] reads no texture count through a null `tex` and
//! answers as if every texture array were empty; `graphics.c` always passes
//! its local array.

use core::ffi::c_int;

use crate::immediate::{self as core_immediate, Draw, Save, TEX_ARRAYS};

/// `struct gs_immediate_draw` (`graphics-immediate.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct gs_immediate_draw {
    pub num: usize,
    pub norms_mismatch: bool,
    pub colors_mismatch: bool,
    pub tex_mismatch: bool,
}

impl From<Draw> for gs_immediate_draw {
    fn from(d: Draw) -> Self {
        Self {
            num: d.num,
            norms_mismatch: d.norms_mismatch,
            colors_mismatch: d.colors_mismatch,
            tex_mismatch: d.tex_mismatch,
        }
    }
}

/// `struct gs_immediate_save` (`graphics-immediate.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct gs_immediate_save {
    pub num: usize,
    pub num_tex: usize,
    pub free_from: usize,
}

impl From<Save> for gs_immediate_save {
    fn from(s: Save) -> Self {
        Self {
            num: s.num,
            num_tex: s.num_tex,
            free_from: s.free_from,
        }
    }
}

/// `bool gs_immediate_unit_ok(bool immediate, int unit)`.
#[unsafe(no_mangle)]
pub extern "C" fn gs_immediate_unit_ok(immediate: bool, unit: c_int) -> bool {
    core_immediate::unit_ok(immediate, unit)
}

/// `bool gs_immediate_push_ok(bool immediate, size_t num)`.
#[unsafe(no_mangle)]
pub extern "C" fn gs_immediate_push_ok(immediate: bool, num: usize) -> bool {
    core_immediate::push_ok(immediate, num)
}

/// `struct gs_immediate_draw gs_immediate_draw(size_t verts, size_t norms,
/// size_t colors, size_t tex0)`.
#[unsafe(no_mangle)]
pub extern "C" fn gs_immediate_draw(
    verts: usize,
    norms: usize,
    colors: usize,
    tex0: usize,
) -> gs_immediate_draw {
    core_immediate::draw(verts, norms, colors, tex0).into()
}

/// `struct gs_immediate_save gs_immediate_save(size_t verts, size_t norms,
/// size_t colors, const size_t tex[GS_IMMEDIATE_TEX_ARRAYS])`.
///
/// # Safety
///
/// `tex` is null or points to `GS_IMMEDIATE_TEX_ARRAYS` readable `size_t`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_immediate_save(
    verts: usize,
    norms: usize,
    colors: usize,
    tex: *const usize,
) -> gs_immediate_save {
    // SAFETY: the caller passes null or GS_IMMEDIATE_TEX_ARRAYS counts.
    let counts = unsafe { tex.cast::<[usize; TEX_ARRAYS]>().as_ref() }
        .copied()
        .unwrap_or([0; TEX_ARRAYS]);
    core_immediate::save(verts, norms, colors, &counts).into()
}
