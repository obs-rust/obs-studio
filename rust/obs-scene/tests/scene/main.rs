//! obs-scene integration tests, one module per extracted C file.

mod atomic_update;
mod atomic_update_parity;
mod audio_actions;
mod audio_actions_parity;
mod audio_mix;
mod audio_mix_parity;
mod audio_window;
mod audio_window_parity;
mod blend;
mod bounds;
mod coords;
mod crop;
mod crop_parity;
mod extent;
mod extent_parity;
mod from_source;
mod from_source_parity;
mod item_audio_actions;
mod item_audio_actions_parity;
mod item_id;
mod item_id_parity;
mod item_names;
mod item_names_parity;
mod item_order;
mod item_order_parity;
mod item_parent_getter;
mod item_parent_getter_parity;
mod item_transform;
mod item_transform_parity;
mod kind;
mod kind_parity;
mod render_plan;
mod render_plan_parity;
mod source_getters;
mod source_getters_parity;
mod source_type_queries;
mod source_type_queries_parity;
mod transform_update;
mod transform_update_parity;

mod bounds_place;
mod bounds_place_parity;
mod group_queries;
mod group_queries_parity;
mod group_resize_defer;
mod group_resize_defer_parity;
mod group_transform;
mod group_transform_parity;

mod item_state_getters;

mod item_state_getters_parity;

mod item_render_getters;

mod item_bounds_getters;
mod item_render_getters_parity;

mod item_bounds_getters_parity;
mod item_scale_getter;
mod item_scale_getter_parity;

mod item_transform_getters;

mod item_crop_getter;
#[allow(unsafe_code)]
mod item_transform_getters_parity;

mod item_crop_getter_parity;

mod item_transition_getters;

mod item_transition_getters_parity;
mod item_transition_setters;
mod item_transition_setters_parity;

mod item_group_scene;
mod item_group_scene_parity;
mod item_visible;
mod item_visible_parity;

mod item_blending_method;
mod item_blending_method_parity;
mod item_blending_mode;
mod item_blending_mode_parity;
mod item_locked_setter;
mod item_locked_setter_parity;
mod item_scale_filter;
mod item_scale_filter_parity;
mod item_transition_serialization;

mod item_transition_serialization_parity;
mod lookup;

mod lookup_parity;

mod item_select;
mod item_select_parity;

mod create;
mod create_parity;
mod refs;
mod refs_parity;

mod transform_states;
mod transform_states_parity;
