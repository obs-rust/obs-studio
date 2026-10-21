//! C ABI for `libobs/obs-scene-blend.c.h`.
//!
//! The C file is one table, `obs_blend_mode_params`, so the shim is that
//! table: the same rows as data, built at compile time from the safe core.
//! `render_item_texture()` in `obs-scene.c` indexes it by the item's blend
//! mode unchecked, as before. A mode past `OBS_BLEND_DARKEN` reads past the
//! end of the table in C and here alike; `obs_sceneitem_set_blending_mode()`
//! stores any value. The C file has no `EXPORT`, so the symbol stays hidden.

use core::ffi::c_int;

use crate::blend::{BlendParams, MODE_COUNT, blend_mode_params};

/// `struct obs_blend_mode_param` in `libobs/obs-scene-blend.h`: five enums,
/// each a C `int`. Size 20, align 4, fields at 0, 4, 8, 12 and 16.
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct obs_blend_mode_param {
    /// `enum gs_blend_type`.
    pub src_color: c_int,
    /// `enum gs_blend_type`.
    pub src_alpha: c_int,
    /// `enum gs_blend_type`.
    pub dst_color: c_int,
    /// `enum gs_blend_type`.
    pub dst_alpha: c_int,
    /// `enum gs_blend_op_type`.
    pub op: c_int,
}

impl obs_blend_mode_param {
    const fn from_params(p: BlendParams) -> Self {
        Self {
            src_color: p.src_color as c_int,
            src_alpha: p.src_alpha as c_int,
            dst_color: p.dst_color as c_int,
            dst_alpha: p.dst_alpha as c_int,
            op: p.op as c_int,
        }
    }
}

const fn c_table() -> [obs_blend_mode_param; MODE_COUNT] {
    let rows = blend_mode_params();
    let mut out = [obs_blend_mode_param::from_params(rows[0]); MODE_COUNT];
    let mut i = 1;
    while i < MODE_COUNT {
        out[i] = obs_blend_mode_param::from_params(rows[i]);
        i += 1;
    }
    out
}

/// Same contents as the C `obs_blend_mode_params`, indexed by
/// `enum obs_blending_type`.
#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
pub static obs_blend_mode_params: [obs_blend_mode_param; MODE_COUNT] = c_table();
