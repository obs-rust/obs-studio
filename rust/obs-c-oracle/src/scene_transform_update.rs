//! `oracle_*` build of `libobs/obs-scene-transform-update.c.h`
//! (oracle/scene_transform_update.c): the transform update decisions of
//! `scene_video_render()` and `update_transforms_and_prune_sources()`,
//! renamed. Everything crosses as `uint32_t` and `bool`.

unsafe extern "C" {
    pub fn oracle_obs_scene_transform_update_scene_size(
        last_width: *mut u32,
        last_height: *mut u32,
        width: u32,
        height: u32,
    ) -> bool;

    pub fn oracle_obs_scene_transform_update_item(
        update_transform: bool,
        last_width: u32,
        last_height: u32,
        width: u32,
        height: u32,
        scene_size_changed: bool,
    ) -> bool;
}
