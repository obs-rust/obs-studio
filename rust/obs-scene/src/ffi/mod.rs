//! C ABI shims, one module per C file.
pub mod group_queries;
pub mod item_id;

pub mod scene_audio_actions;
pub mod scene_audio_mix;
pub mod scene_audio_window;
pub mod scene_blend;
pub mod scene_bounds;
pub mod scene_coords;
pub mod scene_crop;
pub mod scene_extent;
pub mod scene_group_transform;
pub mod scene_item_names;
pub mod scene_item_order;
pub mod scene_item_transform;
pub mod scene_render_plan;
pub mod scene_transform_update;
pub mod source_type_queries;

pub mod scene_bounds_place;

pub mod item_state_getters;

pub mod item_bounds_getters;
pub mod item_render_getters;
pub mod item_scale_getter;

pub mod item_crop_getter;
pub mod item_transform_getters;

pub mod item_transition_getters;
