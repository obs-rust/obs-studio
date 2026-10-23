//! `oracle_*` build of `libobs/obs-scene-extent.c.h`
//! (oracle/scene_extent.c).

use super::graphics_math::OracleMatrix4;
use super::vec2::OracleVec2;

unsafe extern "C" {
    pub fn oracle_obs_scene_extent_init(minv: *mut OracleVec2, maxv: *mut OracleVec2);

    pub fn oracle_obs_scene_extent_add(
        minv: *mut OracleVec2,
        maxv: *mut OracleVec2,
        box_transform: *const OracleMatrix4,
    );

    pub fn oracle_obs_scene_extent_size(
        scale: *mut OracleVec2,
        cx: *mut u32,
        cy: *mut u32,
        minv: *const OracleVec2,
        maxv: *const OracleVec2,
    );
}
