//! `oracle_*` build of `libobs/obs-scene-blend.c.h`
//! (oracle/scene_blend.c).

use core::ffi::c_int;

/// `struct obs_blend_mode_param` in `libobs/obs-scene-blend.h`. The parity
/// test checks this layout against `oracle_scene_blend_layout`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleBlendModeParam {
    pub src_color: c_int,
    pub src_alpha: c_int,
    pub dst_color: c_int,
    pub dst_alpha: c_int,
    pub op: c_int,
}

/// `kind` of `oracle_scene_blend_enum`.
pub const ENUM_BLENDING_TYPE: c_int = 0;
pub const ENUM_BLEND_TYPE: c_int = 1;
pub const ENUM_BLEND_OP: c_int = 2;

unsafe extern "C" {
    /// Rows in the C table.
    pub fn oracle_scene_blend_rows() -> usize;
    /// The C table (`oracle_scene_blend_rows()` rows).
    pub fn oracle_scene_blend_table() -> *const OracleBlendModeParam;
    /// sizeof, alignment, then the five field offsets; `usize::MAX` past
    /// the end.
    pub fn oracle_scene_blend_layout(which: usize) -> usize;
    /// Value `which` of enum `kind` in declaration order; -1 past the end.
    pub fn oracle_scene_blend_enum(kind: c_int, which: usize) -> c_int;
}
