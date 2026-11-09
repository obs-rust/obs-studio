//! Safe core of `libobs/obs-display.c.h`: the display's life and its render.
//!
//! - [`init`] and [`free`]: `obs_display_init()` and `obs_display_free()`,
//!   the mutexes, the clear workaround, the swap chain and the sizes;
//! - [`create`] and [`destroy`]: `obs_display_create()` and
//!   `obs_display_destroy()`, the allocation and the core's display list;
//! - [`resize`] and [`update_color_space`]: the size and colour-space
//!   handoff to the graphics thread, under `draw_info_mutex`;
//! - [`add_draw_callback`] and [`remove_draw_callback`], under
//!   `draw_callbacks_mutex`;
//! - [`render`]: `render_display()`, with `render_display_begin()` and
//!   `render_display_end()` folded in, which `render_displays()` calls for
//!   each display on every frame of the graphics thread.
//!
//! Every field read and write, lock, list link, allocation, log line and
//! libobs or graphics call is one [`Host`] method, called where and under
//! the condition the C has it, so a field the C reads twice is read twice.
//! The clear rules are [`crate::canvas_draw`]'s, called directly. The colour
//! conversions of `graphics/vec4.h` and `graphics/srgb.h` that the clear
//! colour needs (`vec4_from_rgba()`, `vec4_from_rgba_srgb()`), which are
//! `static inline` with no symbol, are ported here.
//!
//! What the C leaves as it is stays so: a draw callback runs with
//! `draw_callbacks_mutex` held, so one that adds or removes a callback on
//! its own display deadlocks; `draw_info_mutex` is let go before
//! `draw_callbacks_mutex` is taken; `cx` and `cy` take the new size before
//! `gs_is_present_ready()` is asked; a create that fails destroys the
//! display inside `gs_enter_context()`, and the destroy enters graphics
//! again.
//!
//! No input comes from outside OBS: the sizes and colours come from the
//! frontend and plugins, the callbacks from plugins. Nothing here can panic
//! on any input.
//!
//! No intentional differences from the C.

#![allow(unused_variables, reason = "RED: the bodies come with the port")]
#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

use core::ffi::{CStr, c_int};

/// `"obs_display_init: Failed to create swap chain"`, logged at
/// `LOG_ERROR`.
pub const SWAP_CHAIN_FAILED: &CStr = c"obs_display_init: Failed to create swap chain";
/// `"obs_display_init: Failed to create mutex"`, logged at `LOG_ERROR`.
pub const MUTEX_FAILED: &CStr = c"obs_display_init: Failed to create mutex";
/// The solid effect's colour parameter.
pub const COLOR_PARAM: &CStr = c"color";
/// The solid effect's technique.
pub const SOLID_TECHNIQUE: &CStr = c"Solid";

/// `GS_NEITHER` (`enum gs_cull_mode`).
pub const GS_NEITHER: c_int = 2;

/// `struct vec4`: four floats, x to w.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec4 {
    /// `x`, red here.
    pub x: f32,
    /// `y`, green.
    pub y: f32,
    /// `z`, blue.
    pub z: f32,
    /// `w`, alpha.
    pub w: f32,
}

/// `gs_srgb_nonlinear_to_linear()` (`graphics/srgb.h`).
#[must_use]
pub fn srgb_nonlinear_to_linear(u: f32) -> f32 {
    todo!()
}

/// `vec4_from_rgba()` (`graphics/vec4.h`): the four bytes of `rgba` in
/// memory order (`memcpy()`), each over 255.
#[must_use]
pub fn vec4_from_rgba(rgba: u32) -> Vec4 {
    todo!()
}

/// `vec4_from_rgba_srgb()` (`graphics/vec4.h`): [`vec4_from_rgba`], then
/// the first three to linear.
#[must_use]
pub fn vec4_from_rgba_srgb(rgba: u32) -> Vec4 {
    todo!()
}

/// `(float)cx`: round to nearest, as C converts.
#[must_use]
pub fn to_float(value: u32) -> f32 {
    todo!()
}

/// `(int)cx`: the same bits, as the C compilers OBS builds with convert.
#[must_use]
pub fn to_int(value: u32) -> c_int {
    todo!()
}

/// Everything the display reaches: the fields of `struct obs_display` and
/// the core (through the display seam), and the calls into libobs and the
/// graphics subsystem. One method per C read, write, lock, link or call.
pub trait Host {
    /// An `obs_display_t *`, never NULL.
    type Display: Copy + PartialEq;
    /// A `const struct gs_init_data *`, never NULL.
    type Data: Copy;
    /// A `gs_swapchain_t *`, never NULL.
    type Swap: Copy;
    /// An `obs_display_t **`, a link of the display list, never NULL.
    type Link: Copy + PartialEq;
    /// A `struct draw_callback`.
    type Callback: Copy;
    /// A `graphics_t *`, as the core holds it.
    type Graphics: Copy;
    /// A `gs_effect_t *`, as the core holds it.
    type Effect: Copy;
    /// A `gs_eparam_t *`, as the lookup gives it.
    type Param: Copy;

    /// `bzalloc(sizeof(struct obs_display))`.
    fn alloc(&mut self) -> Self::Display;
    /// `bfree(display)`.
    fn dealloc(&mut self, display: Self::Display);
    /// `blog(LOG_ERROR, message)`.
    fn log_error(&mut self, message: &'static CStr);

    /// `pthread_mutex_init_value()` of both mutexes.
    fn init_mutex_values(&mut self, display: Self::Display);
    /// `pthread_mutex_init(&display->draw_callbacks_mutex, NULL)`.
    fn callbacks_mutex_init(&mut self, display: Self::Display) -> c_int;
    /// `pthread_mutex_init(&display->draw_info_mutex, NULL)`.
    fn info_mutex_init(&mut self, display: Self::Display) -> c_int;
    /// `pthread_mutex_destroy(&display->draw_callbacks_mutex)`.
    fn callbacks_mutex_destroy(&mut self, display: Self::Display);
    /// `pthread_mutex_destroy(&display->draw_info_mutex)`.
    fn info_mutex_destroy(&mut self, display: Self::Display);
    /// `pthread_mutex_lock(&display->draw_callbacks_mutex)`.
    fn lock_callbacks(&mut self, display: Self::Display);
    /// `pthread_mutex_unlock(&display->draw_callbacks_mutex)`.
    fn unlock_callbacks(&mut self, display: Self::Display);
    /// `pthread_mutex_lock(&display->draw_info_mutex)`.
    fn lock_info(&mut self, display: Self::Display);
    /// `pthread_mutex_unlock(&display->draw_info_mutex)`.
    fn unlock_info(&mut self, display: Self::Display);

    /// `obs_host_display_clear_workaround()`.
    fn host_clear_workaround(&mut self) -> bool;
    /// `obs_host_display_clear_color_always_srgb()`.
    fn host_clear_color_always_srgb(&mut self) -> bool;

    /// `display->use_clear_workaround`.
    fn use_clear_workaround(&mut self, display: Self::Display) -> bool;
    /// `display->use_clear_workaround = value`.
    fn set_use_clear_workaround(&mut self, display: Self::Display, value: bool);
    /// `display->swap`, `None` for NULL.
    fn swap(&mut self, display: Self::Display) -> Option<Self::Swap>;
    /// `display->swap = value`.
    fn set_swap(&mut self, display: Self::Display, value: Option<Self::Swap>);
    /// `display->cx`.
    fn cx(&mut self, display: Self::Display) -> u32;
    /// `display->cy`.
    fn cy(&mut self, display: Self::Display) -> u32;
    /// `display->cx = value`.
    fn set_cx(&mut self, display: Self::Display, value: u32);
    /// `display->cy = value`.
    fn set_cy(&mut self, display: Self::Display, value: u32);
    /// `display->next_cx`.
    fn next_cx(&mut self, display: Self::Display) -> u32;
    /// `display->next_cy`.
    fn next_cy(&mut self, display: Self::Display) -> u32;
    /// `display->next_cx = value`.
    fn set_next_cx(&mut self, display: Self::Display, value: u32);
    /// `display->next_cy = value`.
    fn set_next_cy(&mut self, display: Self::Display, value: u32);
    /// `display->update_color_space`.
    fn update_color_space(&mut self, display: Self::Display) -> bool;
    /// `display->update_color_space = value`.
    fn set_update_color_space(&mut self, display: Self::Display, value: bool);
    /// `display->enabled`.
    fn enabled(&mut self, display: Self::Display) -> bool;
    /// `display->enabled = value`.
    fn set_enabled(&mut self, display: Self::Display, value: bool);
    /// `display->background_color`.
    fn background_color(&mut self, display: Self::Display) -> u32;
    /// `display->background_color = value`.
    fn set_background_color(&mut self, display: Self::Display, value: u32);

    /// `da_push_back(display->draw_callbacks, &callback)`.
    fn callbacks_push(&mut self, display: Self::Display, callback: Self::Callback);
    /// `da_erase_item(display->draw_callbacks, &callback)`.
    fn callbacks_erase_item(&mut self, display: Self::Display, callback: Self::Callback);
    /// `display->draw_callbacks.num`.
    fn callbacks_num(&mut self, display: Self::Display) -> usize;
    /// `display->draw_callbacks.array[idx]`, `idx` below the count.
    fn callbacks_get(&mut self, display: Self::Display, idx: usize) -> Self::Callback;
    /// `da_free(display->draw_callbacks)`.
    fn callbacks_free(&mut self, display: Self::Display);
    /// `callback->draw(callback->param, cx, cy)`.
    fn call(&mut self, callback: Self::Callback, cx: u32, cy: u32);

    /// `pthread_mutex_lock(&obs->data.displays_mutex)`.
    fn lock_displays(&mut self);
    /// `pthread_mutex_unlock(&obs->data.displays_mutex)`.
    fn unlock_displays(&mut self);
    /// `obs->data.first_display`.
    fn first_display(&mut self) -> Option<Self::Display>;
    /// `obs->data.first_display = value`.
    fn set_first_display(&mut self, value: Option<Self::Display>);
    /// `&obs->data.first_display`.
    fn first_display_ref(&mut self) -> Self::Link;
    /// `display->next`.
    fn next(&mut self, display: Self::Display) -> Option<Self::Display>;
    /// `display->next = value`.
    fn set_next(&mut self, display: Self::Display, value: Option<Self::Display>);
    /// `&display->next`.
    fn next_ref(&mut self, display: Self::Display) -> Self::Link;
    /// `display->prev_next`.
    fn prev_next(&mut self, display: Self::Display) -> Option<Self::Link>;
    /// `display->prev_next = value`.
    fn set_prev_next(&mut self, display: Self::Display, value: Option<Self::Link>);
    /// `*link = value`.
    fn store_link(&mut self, link: Self::Link, value: Option<Self::Display>);

    /// `obs->video.graphics`.
    fn graphics(&mut self) -> Self::Graphics;
    /// `obs->video.solid_effect`.
    fn solid_effect(&mut self) -> Self::Effect;
    /// `graphics_data->cx`.
    fn data_cx(&mut self, data: Self::Data) -> u32;
    /// `graphics_data->cy`.
    fn data_cy(&mut self, data: Self::Data) -> u32;

    /// `obs_enter_graphics()`.
    fn enter_graphics(&mut self);
    /// `obs_leave_graphics()`.
    fn leave_graphics(&mut self);
    /// `gs_enter_context(graphics)`.
    fn enter_context(&mut self, graphics: Self::Graphics);
    /// `gs_leave_context()`.
    fn leave_context(&mut self);
    /// `gs_swapchain_create(data)`, `None` for NULL.
    fn swapchain_create(&mut self, data: Self::Data) -> Option<Self::Swap>;
    /// `gs_swapchain_destroy(swap)`.
    fn swapchain_destroy(&mut self, swap: Self::Swap);
    /// `gs_load_swapchain(swap)`.
    fn load_swapchain(&mut self, swap: Option<Self::Swap>);
    /// `gs_resize(cx, cy)`.
    fn resize(&mut self, cx: u32, cy: u32);
    /// `gs_update_color_space()`.
    fn gs_update_color_space(&mut self);
    /// `gs_is_present_ready()`.
    fn is_present_ready(&mut self) -> bool;
    /// `gs_begin_scene()`.
    fn begin_scene(&mut self);
    /// `(int)gs_get_color_space()`.
    fn color_space(&mut self) -> c_int;
    /// `gs_clear(flags, &color, depth, stencil)`.
    fn clear(&mut self, flags: u32, color: Vec4, depth: f32, stencil: u8);
    /// `gs_enable_depth_test(enable)`.
    fn enable_depth_test(&mut self, enable: bool);
    /// `gs_set_cull_mode(mode)`.
    fn set_cull_mode(&mut self, mode: c_int);
    /// `gs_ortho(left, right, top, bottom, znear, zfar)`.
    fn ortho(&mut self, left: f32, right: f32, top: f32, bottom: f32, znear: f32, zfar: f32);
    /// `gs_set_viewport(x, y, width, height)`.
    fn set_viewport(&mut self, x: c_int, y: c_int, width: c_int, height: c_int);
    /// `gs_effect_get_param_by_name(effect, name)`.
    fn param_by_name(&mut self, effect: Self::Effect, name: &'static CStr) -> Self::Param;
    /// `gs_effect_set_vec4(param, &value)`.
    fn effect_set_vec4(&mut self, param: Self::Param, value: Vec4);
    /// `gs_effect_loop(effect, technique)`.
    fn effect_loop(&mut self, effect: Self::Effect, technique: &'static CStr) -> bool;
    /// `gs_draw_sprite(NULL, 0, cx, cy)`.
    fn draw_sprite(&mut self, cx: u32, cy: u32);
    /// `gs_end_scene()`.
    fn end_scene(&mut self);
    /// `gs_present()`.
    fn present(&mut self);
}

/// `obs_display_init()`: the mutexes, the clear workaround, and with
/// graphics data the swap chain and the size. `false` when the swap chain or
/// a mutex cannot be made, with its error logged.
pub fn init<H: Host>(host: &mut H, display: H::Display, graphics_data: Option<H::Data>) -> bool {
    todo!()
}

/// `obs_display_create()`: a zeroed display made inside the core's
/// graphics context, linked at the front of the display list, or `None`
/// (destroyed again, still inside the context) when [`init`] fails.
pub fn create<H: Host>(
    host: &mut H,
    graphics_data: Option<H::Data>,
    background_color: u32,
) -> Option<H::Display> {
    todo!()
}

/// `obs_display_free()`: both mutexes, the callbacks and the swap chain.
pub fn free<H: Host>(host: &mut H, display: H::Display) {
    todo!()
}

/// `obs_display_destroy()`: unlinked from the display list, freed inside
/// graphics, and deallocated. Nothing for `None`.
pub fn destroy<H: Host>(host: &mut H, display: Option<H::Display>) {
    todo!()
}

/// `obs_display_resize()`: the size the next render draws at.
pub fn resize<H: Host>(host: &mut H, display: Option<H::Display>, cx: u32, cy: u32) {
    todo!()
}

/// `obs_display_update_color_space()`: the next render updates the swap
/// chain's colour space.
pub fn update_color_space<H: Host>(host: &mut H, display: Option<H::Display>) {
    todo!()
}

/// `obs_display_add_draw_callback()`.
pub fn add_draw_callback<H: Host>(
    host: &mut H,
    display: Option<H::Display>,
    callback: H::Callback,
) {
    todo!()
}

/// `obs_display_remove_draw_callback()`: the first match only.
pub fn remove_draw_callback<H: Host>(
    host: &mut H,
    display: Option<H::Display>,
    callback: H::Callback,
) {
    todo!()
}

/// `render_display()`: nothing for `None` or a disabled display; otherwise
/// the next size and the colour-space flag taken (and the flag cleared)
/// under `draw_info_mutex`, and when the swap chain is ready every draw
/// callback in order under `draw_callbacks_mutex`, the scene ended and the
/// frame presented.
pub fn render<H: Host>(host: &mut H, display: Option<H::Display>) {
    todo!()
}
