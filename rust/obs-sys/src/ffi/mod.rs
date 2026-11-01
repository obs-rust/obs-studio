//! Declarations from the unchanged libobs public headers.
//!
//! The first group is the lifecycle used by the bundled-build contract tests;
//! the second group is the scene, source, output and graphics surface that the
//! in-process MCP backend (`obs-mcp`, #3283) calls.
#![allow(non_camel_case_types, reason = "names mirror the C headers")]

use std::ffi::{c_char, c_double, c_float, c_int, c_void};

unsafe extern "C" {
    /// Returns the libobs C API version from `libobs/obs-config.h`.
    pub fn obs_get_version() -> u32;
    /// Initializes libobs; a null name store asks libobs to create its own.
    pub fn obs_startup(
        locale: *const std::ffi::c_char,
        module_config_path: *const std::ffi::c_char,
        store: *mut std::ffi::c_void,
    ) -> bool;
    pub fn obs_initialized() -> bool;
    pub fn obs_shutdown();
}

macro_rules! opaque {
    ($($name:ident),* $(,)?) => {$(
        /// Opaque libobs handle.
        #[repr(C)]
        pub struct $name {
            _private: [u8; 0],
        }
    )*};
}

opaque!(
    obs_source_t,
    obs_scene_t,
    obs_sceneitem_t,
    obs_data_t,
    obs_output_t,
    video_t,
    os_cpu_usage_info_t,
    gs_texrender_t,
    gs_texture_t,
    gs_stagesurf_t,
);

/// `struct vec2` from `libobs/graphics/vec2.h`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct vec2 {
    pub x: c_float,
    pub y: c_float,
}

/// `struct vec4` from `libobs/graphics/vec4.h` (a 16-byte aligned union).
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default)]
pub struct vec4 {
    pub x: c_float,
    pub y: c_float,
    pub z: c_float,
    pub w: c_float,
}

/// `OBS_SOURCE_TYPE_SCENE` of `enum obs_source_type`.
pub const OBS_SOURCE_TYPE_SCENE: c_int = 3;
/// `GS_RGBA` of `enum gs_color_format`.
pub const GS_RGBA: c_int = 3;
/// `GS_ZS_NONE` of `enum gs_zstencil_format`.
pub const GS_ZS_NONE: c_int = 0;
/// `GS_CLEAR_COLOR`.
pub const GS_CLEAR_COLOR: u32 = 1;
/// `GS_BLEND_ZERO` of `enum gs_blend_type`.
pub const GS_BLEND_ZERO: c_int = 0;
/// `GS_BLEND_ONE` of `enum gs_blend_type`.
pub const GS_BLEND_ONE: c_int = 1;

/// Callback of `obs_enum_sources` and `obs_enum_scenes`; return `true` to continue.
pub type obs_enum_proc =
    unsafe extern "C" fn(param: *mut c_void, source: *mut obs_source_t) -> bool;

unsafe extern "C" {
    // libobs/obs.h: sources and scenes.
    pub fn obs_enum_sources(proc_: obs_enum_proc, param: *mut c_void);
    pub fn obs_enum_scenes(proc_: obs_enum_proc, param: *mut c_void);
    pub fn obs_get_source_by_name(name: *const c_char) -> *mut obs_source_t;
    pub fn obs_source_create(
        id: *const c_char,
        name: *const c_char,
        settings: *mut obs_data_t,
        hotkey_data: *mut obs_data_t,
    ) -> *mut obs_source_t;
    pub fn obs_source_get_ref(source: *mut obs_source_t) -> *mut obs_source_t;
    pub fn obs_source_release(source: *mut obs_source_t);
    pub fn obs_source_remove(source: *mut obs_source_t);
    pub fn obs_source_update(source: *mut obs_source_t, settings: *mut obs_data_t);
    pub fn obs_source_get_name(source: *const obs_source_t) -> *const c_char;
    pub fn obs_source_get_id(source: *const obs_source_t) -> *const c_char;
    pub fn obs_source_get_type(source: *const obs_source_t) -> c_int;
    pub fn obs_source_get_width(source: *mut obs_source_t) -> u32;
    pub fn obs_source_get_height(source: *mut obs_source_t) -> u32;
    pub fn obs_source_video_render(source: *mut obs_source_t);
    pub fn obs_source_inc_showing(source: *mut obs_source_t);
    pub fn obs_source_dec_showing(source: *mut obs_source_t);
    pub fn obs_scene_create(name: *const c_char) -> *mut obs_scene_t;
    pub fn obs_scene_release(scene: *mut obs_scene_t);
    pub fn obs_scene_from_source(source: *const obs_source_t) -> *mut obs_scene_t;
    pub fn obs_scene_get_source(scene: *const obs_scene_t) -> *mut obs_source_t;
    pub fn obs_scene_add(
        scene: *mut obs_scene_t,
        source: *mut obs_source_t,
    ) -> *mut obs_sceneitem_t;
    pub fn obs_scene_find_source(
        scene: *mut obs_scene_t,
        name: *const c_char,
    ) -> *mut obs_sceneitem_t;
    pub fn obs_sceneitem_release(item: *mut obs_sceneitem_t);
    pub fn obs_sceneitem_visible(item: *const obs_sceneitem_t) -> bool;
    pub fn obs_sceneitem_set_visible(item: *mut obs_sceneitem_t, visible: bool) -> bool;
    pub fn obs_sceneitem_get_pos(item: *const obs_sceneitem_t, pos: *mut vec2);
    pub fn obs_sceneitem_set_pos(item: *mut obs_sceneitem_t, pos: *const vec2);
    pub fn obs_sceneitem_get_scale(item: *const obs_sceneitem_t, scale: *mut vec2);
    pub fn obs_sceneitem_set_scale(item: *mut obs_sceneitem_t, scale: *const vec2);
    pub fn obs_sceneitem_get_rot(item: *const obs_sceneitem_t) -> c_float;
    pub fn obs_sceneitem_set_rot(item: *mut obs_sceneitem_t, rot_deg: c_float);
    pub fn obs_set_output_source(channel: u32, source: *mut obs_source_t);
    pub fn obs_get_output_source(channel: u32) -> *mut obs_source_t;
    pub fn obs_wait_for_destroy_queue() -> bool;

    // libobs/obs-data.h
    pub fn obs_data_create_from_json(json: *const c_char) -> *mut obs_data_t;
    pub fn obs_data_release(data: *mut obs_data_t);

    // libobs/obs.h: outputs.
    pub fn obs_get_output_by_name(name: *const c_char) -> *mut obs_output_t;
    pub fn obs_output_release(output: *mut obs_output_t);
    pub fn obs_output_start(output: *mut obs_output_t) -> bool;
    pub fn obs_output_stop(output: *mut obs_output_t);
    pub fn obs_output_active(output: *const obs_output_t) -> bool;
    pub fn obs_output_get_total_bytes(output: *const obs_output_t) -> u64;
    pub fn obs_output_get_total_frames(output: *const obs_output_t) -> c_int;
    pub fn obs_output_get_last_error(output: *mut obs_output_t) -> *const c_char;

    // libobs/obs.h: video and statistics.
    pub fn obs_get_video() -> *mut video_t;
    pub fn obs_get_active_fps() -> c_double;
    pub fn obs_get_lagged_frames() -> u32;
    pub fn video_output_get_frame_rate(video: *const video_t) -> c_double;
    pub fn video_output_get_skipped_frames(video: *const video_t) -> u32;
    pub fn os_cpu_usage_info_start() -> *mut os_cpu_usage_info_t;
    pub fn os_cpu_usage_info_query(info: *mut os_cpu_usage_info_t) -> c_double;
    pub fn os_cpu_usage_info_destroy(info: *mut os_cpu_usage_info_t);

    // libobs/obs.h and libobs/graphics/graphics.h: rendering a source.
    pub fn obs_enter_graphics();
    pub fn obs_leave_graphics();
    pub fn gs_texrender_create(format: c_int, zsformat: c_int) -> *mut gs_texrender_t;
    pub fn gs_texrender_destroy(texrender: *mut gs_texrender_t);
    pub fn gs_texrender_begin(texrender: *mut gs_texrender_t, cx: u32, cy: u32) -> bool;
    pub fn gs_texrender_end(texrender: *mut gs_texrender_t);
    pub fn gs_texrender_get_texture(texrender: *const gs_texrender_t) -> *mut gs_texture_t;
    pub fn gs_stagesurface_create(width: u32, height: u32, format: c_int) -> *mut gs_stagesurf_t;
    pub fn gs_stagesurface_destroy(stage: *mut gs_stagesurf_t);
    pub fn gs_stagesurface_map(
        stage: *mut gs_stagesurf_t,
        data: *mut *mut u8,
        linesize: *mut u32,
    ) -> bool;
    pub fn gs_stagesurface_unmap(stage: *mut gs_stagesurf_t);
    pub fn gs_stage_texture(dst: *mut gs_stagesurf_t, src: *mut gs_texture_t);
    pub fn gs_clear(flags: u32, color: *const vec4, depth: c_float, stencil: u8);
    pub fn gs_ortho(
        left: c_float,
        right: c_float,
        top: c_float,
        bottom: c_float,
        znear: c_float,
        zfar: c_float,
    );
    pub fn gs_blend_state_push();
    pub fn gs_blend_state_pop();
    pub fn gs_blend_function(src: c_int, dest: c_int);
}
