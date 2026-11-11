//! `oracle_*` build of `libobs/obs-display.c.h` (oracle/display.c): the
//! display's nine functions under `oracle_` names, on the C oracle of the
//! clear rules, with the display seam (`libobs/obs-display-seam.c.h`)
//! compiled under its own names, which the Rust shim calls too.
//!
//! A world is a stand-in core with its display list. [`oracle_display_use`]
//! makes one the core the seam and the stand-ins see; the stand-ins
//! (`oracle_display_*`, the graphics and libobs calls) record every call in
//! the world's log. One world runs at a time: the caller serializes. Worlds
//! and displays cross as opaque pointers.

use core::ffi::{c_char, c_int, c_void};

/// `struct oracle_display_config`: what the stand-ins answer.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Config {
    /// `obs_host_display_clear_workaround()`.
    pub workaround: bool,
    /// `obs_host_display_clear_color_always_srgb()`.
    pub always_srgb: bool,
    /// `gs_is_present_ready()`.
    pub present_ready: bool,
    /// `gs_swapchain_create()` gives NULL.
    pub swap_fails: bool,
    /// `gs_get_color_space()`.
    pub space: c_int,
    /// How many times `gs_effect_loop()` answers true before each false.
    pub loops: u32,
}

/// A draw callback's function.
pub type DrawFn = unsafe extern "C" fn(param: *mut c_void, cx: u32, cy: u32);

unsafe extern "C" {
    /// A world with an empty display list.
    pub fn oracle_display_new() -> *mut c_void;
    /// Frees a world (not its displays).
    pub fn oracle_display_free(world: *mut c_void);
    /// Makes `world` the core and the log the stand-ins use.
    pub fn oracle_display_use(world: *mut c_void);
    /// Sets the stand-ins' answers.
    pub fn oracle_display_configure(world: *mut c_void, config: *const Config);
    /// The world's `struct gs_init_data`, at `cx` x `cy`.
    pub fn oracle_display_data(world: *mut c_void, cx: u32, cy: u32) -> *const c_void;
    /// The param of callback `id` (0 to 3).
    pub fn oracle_display_param(world: *mut c_void, id: c_int) -> *mut c_void;
    /// The display the next render runs on, for the callback's lock check.
    pub fn oracle_display_rendering(world: *mut c_void, display: *mut c_void);
    /// The recording draw callback.
    pub fn oracle_display_draw(param: *mut c_void, cx: u32, cy: u32);
    /// A second recording draw callback.
    pub fn oracle_display_draw2(param: *mut c_void, cx: u32, cy: u32);
    /// `display->enabled = enabled`.
    pub fn oracle_display_set_enabled(display: *mut c_void, enabled: bool);
    /// Copies the log out and clears it; its length.
    pub fn oracle_display_take_log(world: *mut c_void, out: *mut c_char, cap: usize) -> usize;
    /// The list and every display's fields; its length.
    pub fn oracle_display_state(
        world: *mut c_void,
        displays: *const *mut c_void,
        num: usize,
        out: *mut c_char,
        cap: usize,
    ) -> usize;
    /// Sizes, alignments and offsets: `struct draw_callback`, `struct vec4`,
    /// `sizeof(struct obs_display)`, then `struct oracle_display_config`.
    pub fn oracle_display_layout(index: usize) -> usize;

    /// The original `obs_display_init()`.
    pub fn oracle_obs_display_init(display: *mut c_void, data: *const c_void) -> bool;
    /// The original `obs_display_create()`.
    pub fn oracle_obs_display_create(data: *const c_void, background_color: u32) -> *mut c_void;
    /// The original `obs_display_free()`.
    pub fn oracle_obs_display_free(display: *mut c_void);
    /// The original `obs_display_destroy()`.
    pub fn oracle_obs_display_destroy(display: *mut c_void);
    /// The original `obs_display_resize()`.
    pub fn oracle_obs_display_resize(display: *mut c_void, cx: u32, cy: u32);
    /// The original `obs_display_update_color_space()`.
    pub fn oracle_obs_display_update_color_space(display: *mut c_void);
    /// The original `obs_display_add_draw_callback()`.
    pub fn oracle_obs_display_add_draw_callback(
        display: *mut c_void,
        draw: Option<DrawFn>,
        param: *mut c_void,
    );
    /// The original `obs_display_remove_draw_callback()`.
    pub fn oracle_obs_display_remove_draw_callback(
        display: *mut c_void,
        draw: Option<DrawFn>,
        param: *mut c_void,
    );
    /// The original `render_display()`.
    pub fn oracle_render_display(display: *mut c_void);

    /// Recording `blog()`.
    pub fn oracle_display_blog(level: c_int, format: *const c_char, ...);
    /// Recording `obs_host_display_clear_workaround()`.
    pub fn oracle_display_host_clear_workaround() -> bool;
    /// Recording `obs_host_display_clear_color_always_srgb()`.
    pub fn oracle_display_host_clear_color_always_srgb() -> bool;
    /// Recording `obs_enter_graphics()`.
    pub fn oracle_display_enter_graphics();
    /// Recording `obs_leave_graphics()`.
    pub fn oracle_display_leave_graphics();
    /// Recording `gs_enter_context()`.
    pub fn oracle_display_enter_context(graphics: *mut c_void);
    /// Recording `gs_leave_context()`.
    pub fn oracle_display_leave_context();
    /// Recording `gs_swapchain_create()`.
    pub fn oracle_display_swapchain_create(data: *const c_void) -> *mut c_void;
    /// Recording `gs_swapchain_destroy()`.
    pub fn oracle_display_swapchain_destroy(swap: *mut c_void);
    /// Recording `gs_load_swapchain()`.
    pub fn oracle_display_load_swapchain(swap: *mut c_void);
    /// Recording `gs_resize()`.
    pub fn oracle_display_resize(cx: u32, cy: u32);
    /// Recording `gs_update_color_space()`.
    pub fn oracle_display_update_color_space();
    /// Recording `gs_is_present_ready()`.
    pub fn oracle_display_is_present_ready() -> bool;
    /// Recording `gs_begin_scene()`.
    pub fn oracle_display_begin_scene();
    /// Recording `gs_get_color_space()`.
    pub fn oracle_display_get_color_space() -> c_int;
    /// Recording `gs_clear()`; `color` is a `struct vec4`.
    pub fn oracle_display_clear(flags: u32, color: *const c_void, depth: f32, stencil: u8);
    /// Recording `gs_enable_depth_test()`.
    pub fn oracle_display_enable_depth_test(enable: bool);
    /// Recording `gs_set_cull_mode()`.
    pub fn oracle_display_set_cull_mode(mode: c_int);
    /// Recording `gs_ortho()`.
    pub fn oracle_display_ortho(
        left: f32,
        right: f32,
        top: f32,
        bottom: f32,
        znear: f32,
        zfar: f32,
    );
    /// Recording `gs_set_viewport()`.
    pub fn oracle_display_set_viewport(x: c_int, y: c_int, width: c_int, height: c_int);
    /// Recording `gs_effect_get_param_by_name()`.
    pub fn oracle_display_get_param_by_name(
        effect: *const c_void,
        name: *const c_char,
    ) -> *mut c_void;
    /// Recording `gs_effect_set_vec4()`; `value` is a `struct vec4`.
    pub fn oracle_display_effect_set_vec4(param: *mut c_void, value: *const c_void);
    /// Recording `gs_effect_loop()`.
    pub fn oracle_display_effect_loop(effect: *mut c_void, technique: *const c_char) -> bool;
    /// Recording `gs_draw_sprite()`.
    pub fn oracle_display_draw_sprite(tex: *mut c_void, flip: u32, width: u32, height: u32);
    /// Recording `gs_end_scene()`.
    pub fn oracle_display_end_scene();
    /// Recording `gs_present()`.
    pub fn oracle_display_present();
}
