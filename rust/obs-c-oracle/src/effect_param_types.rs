//! `oracle_*` build of `libobs/graphics/effect-param-types.c.h`
//! (oracle/effect_param_types.c): the parameter type decisions of the
//! effect parser, renamed, and the header's enums with the sizes and
//! alignments of the types that cross, through
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
    pub const BOOL_SIZE: usize = 21;
    pub const BOOL_ALIGN: usize = 22;
    pub const INT_SIZE: usize = 23;
    pub const INT_ALIGN: usize = 24;
    /// `sizeof(enum obs_effect_param_route)`, the route's C `int`.
    pub const ROUTE_SIZE: usize = 25;
}

unsafe extern "C" {
    pub fn oracle_obs_effect_param_type(type_: *const c_char) -> c_int;
    pub fn oracle_obs_effect_param_route(type_: *const c_char, is_texture: bool) -> c_int;
    pub fn oracle_obs_effect_param_rows_bad(type_: *const c_char, is_float: bool) -> bool;
    pub fn oracle_obs_effect_param_rows(type_: *const c_char, is_float: bool) -> c_int;
    pub fn oracle_obs_effect_param_has_cols(type_: *const c_char, is_float: bool) -> bool;
    pub fn oracle_obs_effect_param_cols_bad(type_: *const c_char, is_float: bool) -> bool;
    pub fn oracle_obs_effect_param_cols(
        type_: *const c_char,
        is_float: bool,
        count: c_int,
    ) -> c_int;
    pub fn oracle_effect_param_types_layout(which: usize) -> usize;
}
