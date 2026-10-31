//! `oracle_*` build of `libobs/graphics/effect-param-types.c.h`
//! (oracle/effect_param_types.c): the parameter type decisions of the
//! effect parser, renamed, and the header's enums with the layout of
//! `struct obs_effect_param_shape`, through
//! `oracle_effect_param_types_layout`.

use std::ffi::{c_char, c_int};

/// Index of each entry in `oracle_effect_param_types_layout`, in the C's
/// order.
pub mod layout {
    /// `GS_SHADER_PARAM_UNKNOWN` to `GS_SHADER_PARAM_TEXTURE`, in enum order.
    pub const PARAM_TYPES: usize = 0;
    /// The number of `GS_SHADER_PARAM_*` values.
    pub const PARAM_TYPE_COUNT: usize = 13;
    /// `OBS_EFFECT_PARAM_ROUTE_TEXTURE` to `OBS_EFFECT_PARAM_ROUTE_INVALID`.
    pub const ROUTES: usize = 13;
    /// The number of `OBS_EFFECT_PARAM_ROUTE_*` values.
    pub const ROUTE_COUNT: usize = 8;
    pub const SHAPE_SIZE: usize = 21;
    pub const SHAPE_ALIGN: usize = 22;
    pub const SHAPE_COUNT: usize = 23;
    pub const SHAPE_BAD_ROWS: usize = 24;
    pub const SHAPE_BAD_COLS: usize = 25;
}

/// `struct obs_effect_param_shape`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleShape {
    pub count: c_int,
    pub bad_rows: bool,
    pub bad_cols: bool,
}

unsafe extern "C" {
    pub fn oracle_obs_effect_param_type(type_: *const c_char) -> c_int;
    pub fn oracle_obs_effect_param_route(type_: *const c_char, is_texture: bool) -> c_int;
    pub fn oracle_obs_effect_param_array_shape(type_: *const c_char, is_float: bool)
    -> OracleShape;
    pub fn oracle_effect_param_types_layout(which: usize) -> usize;
}
