//! C ABI shims, one module per C file.
pub mod atomic_update;
pub mod group_queries;
pub mod group_resize_defer;
pub mod item_audio_actions;
pub mod item_id;
pub mod item_parent_getter;

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
pub mod source_getters;
pub mod source_type_queries;

pub mod scene_bounds_place;

pub mod item_state_getters;

pub mod item_bounds_getters;
pub mod item_render_getters;
pub mod item_scale_getter;

pub mod item_crop_getter;
pub mod item_transform_getters;

pub mod item_transition_getters;
pub mod item_transition_setters;

pub mod item_group_scene;
pub mod item_visible;

pub mod item_blending_method;
pub mod item_blending_mode;
pub mod item_locked_setter;
pub mod item_scale_filter;
pub mod item_transition_serialization;
pub mod lookup;

pub mod item_select;
