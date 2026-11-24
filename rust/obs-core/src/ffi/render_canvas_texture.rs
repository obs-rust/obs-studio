//! C ABI for `libobs/obs-render-canvas-texture.c.h`: `obs_render_main_texture()`,
//! `obs_render_main_texture_src_color_only()`, `obs_render_canvas_texture()`
//! and `obs_render_canvas_texture_src_color_only()` (`EXPORT` in `obs.h`,
//! `rust-exports.d`).
//!
//! Each runs the safe core ([`crate::render_canvas_texture`]) on `Seam`,
//! which reads the main canvas, a canvas's mix and the mix's
//! `texture_rendered`, `render_space` and `render_texture` through the
//! private adapters of `obs-render-canvas-texture-fields.c.h` (one borrowed
//! read each, the flag as an atomic load), so `struct obs_core`,
//! `struct obs_canvas` and `struct obs_core_video_mix` are never mirrored.
//! The libobs and graphics calls go through [`Calls`], so the parity test
//! hands both sides the same recording stand-ins; [`Libobs`] is libobs's
//! own.
//!
//! One intentional difference: a NULL canvas, or a NULL main canvas, draws
//! nothing, where the C reads `canvas->mix` through NULL and crashes.

use core::ffi::{CStr, c_char, c_int, c_void};
use core::marker::PhantomData;
use core::ptr::NonNull;

use crate::canvas_draw::Technique;
use crate::render_canvas_texture::{
    self as draw, Blend, Graphics, OBS_EFFECT_DEFAULT, OVER, Param, SRC_COLOR_ONLY,
};

/// The canvas and mix reads and the libobs and graphics calls of the draw,
/// one C function each.
///
/// # Safety
/// Each method is the C function its doc names, called as the C calls it,
/// on the thread that holds the graphics context.
pub unsafe trait Calls {
    /// `obs_render_canvas_texture_main_canvas()`: `obs->data.main_canvas`.
    fn main_canvas() -> *const c_void;
    /// `obs_render_canvas_texture_mix(canvas)`: `canvas->mix`.
    ///
    /// # Safety
    /// `canvas` is a live canvas.
    unsafe fn mix(canvas: *const c_void) -> *const c_void;
    /// `obs_render_canvas_texture_rendered(mix)`: the atomic load of
    /// `mix->texture_rendered`.
    ///
    /// # Safety
    /// `mix` is a live mix.
    unsafe fn rendered(mix: *const c_void) -> bool;
    /// `obs_render_canvas_texture_space(mix)`: `(int)mix->render_space`.
    ///
    /// # Safety
    /// As [`Calls::rendered`].
    unsafe fn space(mix: *const c_void) -> c_int;
    /// `obs_render_canvas_texture_texture(mix)`: `mix->render_texture`.
    ///
    /// # Safety
    /// As [`Calls::rendered`].
    unsafe fn texture(mix: *const c_void) -> *mut c_void;
    /// `(int)gs_get_color_space()`.
    fn color_space() -> c_int;
    /// `obs_get_video_sdr_white_level()`.
    fn sdr_white_level() -> f32;
    /// `gs_framebuffer_srgb_enabled()`.
    fn framebuffer_srgb_enabled() -> bool;
    /// `gs_enable_framebuffer_srgb(enable)`.
    fn enable_framebuffer_srgb(enable: bool);
    /// `obs_get_base_effect(effect)`.
    fn base_effect(effect: c_int) -> *mut c_void;
    /// `gs_effect_get_param_by_name(effect, name)`.
    ///
    /// # Safety
    /// `effect` is what [`Calls::base_effect`] gave; `name` is a static
    /// NUL-terminated string.
    unsafe fn param_by_name(effect: *mut c_void, name: *const c_char) -> *mut c_void;
    /// `gs_effect_set_texture_srgb(param, texture)`.
    ///
    /// # Safety
    /// `param` is what [`Calls::param_by_name`] gave; `texture` is the mix's.
    unsafe fn set_texture_srgb(param: *mut c_void, texture: *mut c_void);
    /// `gs_effect_set_float(param, value)`.
    ///
    /// # Safety
    /// As [`Calls::set_texture_srgb`].
    unsafe fn set_float(param: *mut c_void, value: f32);
    /// `gs_blend_state_push()`.
    fn blend_state_push();
    /// `gs_blend_function_separate(src_c, dest_c, src_a, dest_a)`.
    fn blend_function_separate(src_c: u32, dest_c: u32, src_a: u32, dest_a: u32);
    /// `gs_effect_loop(effect, technique)`.
    ///
    /// # Safety
    /// As [`Calls::param_by_name`], for the technique's name.
    unsafe fn effect_loop(effect: *mut c_void, technique: *const c_char) -> bool;
    /// `gs_draw_sprite(texture, flip, width, height)`.
    ///
    /// # Safety
    /// `texture` is the mix's.
    unsafe fn draw_sprite(texture: *mut c_void, flip: u32, width: u32, height: u32);
    /// `gs_blend_state_pop()`.
    fn blend_state_pop();
}

/// [`Calls`] through libobs's own functions.
pub enum Libobs {}

unsafe extern "C" {
    fn obs_render_canvas_texture_main_canvas() -> *const c_void;
    fn obs_render_canvas_texture_mix(canvas: *const c_void) -> *const c_void;
    fn obs_render_canvas_texture_rendered(mix: *const c_void) -> bool;
    fn obs_render_canvas_texture_space(mix: *const c_void) -> c_int;
    fn obs_render_canvas_texture_texture(mix: *const c_void) -> *mut c_void;
    fn gs_get_color_space() -> c_int;
    fn gs_framebuffer_srgb_enabled() -> bool;
    fn gs_enable_framebuffer_srgb(enable: bool);
    fn gs_effect_get_param_by_name(effect: *const c_void, name: *const c_char) -> *mut c_void;
    fn gs_effect_set_texture_srgb(param: *mut c_void, val: *mut c_void);
    fn gs_effect_set_float(param: *mut c_void, val: f32);
    fn gs_blend_state_push();
    fn gs_blend_function_separate(src_c: u32, dest_c: u32, src_a: u32, dest_a: u32);
    fn gs_effect_loop(effect: *mut c_void, technique: *const c_char) -> bool;
    fn gs_draw_sprite(tex: *mut c_void, flip: u32, width: u32, height: u32);
    fn gs_blend_state_pop();
}

// SAFETY: each method is the one libobs function of its name, called with
// the caller's values.
unsafe impl Calls for Libobs {
    fn main_canvas() -> *const c_void {
        // SAFETY: reads the initialized core's field, as the C did.
        unsafe { obs_render_canvas_texture_main_canvas() }
    }
    unsafe fn mix(canvas: *const c_void) -> *const c_void {
        // SAFETY: see the trait.
        unsafe { obs_render_canvas_texture_mix(canvas) }
    }
    unsafe fn rendered(mix: *const c_void) -> bool {
        // SAFETY: see the trait.
        unsafe { obs_render_canvas_texture_rendered(mix) }
    }
    unsafe fn space(mix: *const c_void) -> c_int {
        // SAFETY: see the trait.
        unsafe { obs_render_canvas_texture_space(mix) }
    }
    unsafe fn texture(mix: *const c_void) -> *mut c_void {
        // SAFETY: see the trait.
        unsafe { obs_render_canvas_texture_texture(mix) }
    }
    fn color_space() -> c_int {
        // SAFETY: takes nothing.
        unsafe { gs_get_color_space() }
    }
    fn sdr_white_level() -> f32 {
        // SAFETY: the core is live while a canvas draws.
        unsafe { crate::ffi::video_level_queries::obs_get_video_sdr_white_level() }
    }
    fn framebuffer_srgb_enabled() -> bool {
        // SAFETY: takes nothing.
        unsafe { gs_framebuffer_srgb_enabled() }
    }
    fn enable_framebuffer_srgb(enable: bool) {
        // SAFETY: a value only.
        unsafe { gs_enable_framebuffer_srgb(enable) }
    }
    fn base_effect(effect: c_int) -> *mut c_void {
        // SAFETY: a known effect of the live core, borrowed.
        unsafe { crate::ffi::base_effect::obs_get_base_effect(effect) }
    }
    unsafe fn param_by_name(effect: *mut c_void, name: *const c_char) -> *mut c_void {
        // SAFETY: see the trait.
        unsafe { gs_effect_get_param_by_name(effect, name) }
    }
    unsafe fn set_texture_srgb(param: *mut c_void, texture: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { gs_effect_set_texture_srgb(param, texture) }
    }
    unsafe fn set_float(param: *mut c_void, value: f32) {
        // SAFETY: see the trait.
        unsafe { gs_effect_set_float(param, value) }
    }
    fn blend_state_push() {
        // SAFETY: takes nothing.
        unsafe { gs_blend_state_push() }
    }
    fn blend_function_separate(src_c: u32, dest_c: u32, src_a: u32, dest_a: u32) {
        // SAFETY: values only.
        unsafe { gs_blend_function_separate(src_c, dest_c, src_a, dest_a) }
    }
    unsafe fn effect_loop(effect: *mut c_void, technique: *const c_char) -> bool {
        // SAFETY: see the trait.
        unsafe { gs_effect_loop(effect, technique) }
    }
    unsafe fn draw_sprite(texture: *mut c_void, flip: u32, width: u32, height: u32) {
        // SAFETY: see the trait.
        unsafe { gs_draw_sprite(texture, flip, width, height) }
    }
    fn blend_state_pop() {
        // SAFETY: takes nothing.
        unsafe { gs_blend_state_pop() }
    }
}

/// The technique's name in `default.effect`, as
/// `obs_source_draw_technique_name()` maps it.
const fn technique_name(technique: Technique) -> &'static CStr {
    match technique {
        Technique::Draw => c"Draw",
        Technique::DrawMultiply => c"DrawMultiply",
        Technique::DrawTonemap => c"DrawTonemap",
    }
}

/// A non-NULL `obs_canvas_t *` or `struct obs_core_video_mix *`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Object(NonNull<c_void>);

/// A `gs_texture_t *`, `gs_effect_t *` or `gs_eparam_t *` as libobs gave
/// it, NULL included, passed back as it came.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Opaque(*mut c_void);

/// The core's [`Graphics`] over the adapters and `C`.
///
/// Its methods are safe because the core calls them only on the canvas it
/// was given (or the main canvas) and that canvas's mix, both live under
/// the caller's contract; a seam is therefore made only by the `*_with`
/// functions below.
struct Seam<C: Calls> {
    calls: PhantomData<C>,
}

impl<C: Calls> Graphics for Seam<C> {
    type Canvas = Object;
    type Mix = Object;
    type Texture = Opaque;
    type Effect = Opaque;
    type Param = Opaque;

    fn main_canvas(&mut self) -> Option<Object> {
        NonNull::new(C::main_canvas().cast_mut()).map(Object)
    }
    fn mix(&mut self, canvas: Object) -> Option<Object> {
        // SAFETY: a canvas the caller handed in, or the main canvas, live.
        NonNull::new(unsafe { C::mix(canvas.0.as_ptr()) }.cast_mut()).map(Object)
    }
    fn texture_rendered(&mut self, mix: Object) -> bool {
        // SAFETY: the live canvas's mix.
        unsafe { C::rendered(mix.0.as_ptr()) }
    }
    fn render_space(&mut self, mix: Object) -> c_int {
        // SAFETY: as in texture_rendered.
        unsafe { C::space(mix.0.as_ptr()) }
    }
    fn render_texture(&mut self, mix: Object) -> Opaque {
        // SAFETY: as in texture_rendered.
        Opaque(unsafe { C::texture(mix.0.as_ptr()) })
    }
    fn color_space(&mut self) -> c_int {
        C::color_space()
    }
    fn sdr_white_level(&mut self) -> f32 {
        C::sdr_white_level()
    }
    fn framebuffer_srgb_enabled(&mut self) -> bool {
        C::framebuffer_srgb_enabled()
    }
    fn enable_framebuffer_srgb(&mut self, enable: bool) {
        C::enable_framebuffer_srgb(enable);
    }
    fn default_effect(&mut self) -> Opaque {
        Opaque(C::base_effect(OBS_EFFECT_DEFAULT))
    }
    fn param(&mut self, effect: Opaque, param: Param) -> Opaque {
        // SAFETY: the base effect as it came, and a static name.
        Opaque(unsafe { C::param_by_name(effect.0, param.name().as_ptr()) })
    }
    fn set_texture_srgb(&mut self, param: Opaque, texture: Opaque) {
        // SAFETY: the lookup's param and the mix's texture as they came.
        unsafe { C::set_texture_srgb(param.0, texture.0) }
    }
    fn set_float(&mut self, param: Opaque, value: f32) {
        // SAFETY: the lookup's param as it came.
        unsafe { C::set_float(param.0, value) }
    }
    fn blend_state_push(&mut self) {
        C::blend_state_push();
    }
    fn blend_function_separate(&mut self, blend: Blend) {
        C::blend_function_separate(
            blend.src_color,
            blend.dest_color,
            blend.src_alpha,
            blend.dest_alpha,
        );
    }
    fn effect_loop(&mut self, effect: Opaque, technique: Technique) -> bool {
        // SAFETY: the base effect as it came, and a static name.
        unsafe { C::effect_loop(effect.0, technique_name(technique).as_ptr()) }
    }
    fn draw_sprite(&mut self, texture: Opaque) {
        // SAFETY: the mix's texture as it came; 0, 0 is the texture's size.
        unsafe { C::draw_sprite(texture.0, 0, 0, 0) }
    }
    fn blend_state_pop(&mut self) {
        C::blend_state_pop();
    }
}

/// [`obs_render_main_texture`] (`src_color_only` false) or
/// [`obs_render_main_texture_src_color_only`] over `C`.
///
/// # Safety
/// The core is initialized; its main canvas is NULL or live and that
/// canvas's mix NULL or live; the caller holds the graphics context.
pub unsafe fn render_main_with<C: Calls>(src_color_only: bool) {
    let blend = if src_color_only { SRC_COLOR_ONLY } else { OVER };
    draw::render_main(&mut Seam::<C> { calls: PhantomData }, blend);
}

/// [`obs_render_canvas_texture`] (`src_color_only` false) or
/// [`obs_render_canvas_texture_src_color_only`] over `C`.
///
/// # Safety
/// `canvas` is NULL or live and its mix NULL or live; the caller holds the
/// graphics context.
pub unsafe fn render_canvas_with<C: Calls>(canvas: *mut c_void, src_color_only: bool) {
    let blend = if src_color_only { SRC_COLOR_ONLY } else { OVER };
    let canvas = NonNull::new(canvas).map(Object);
    draw::render_canvas(&mut Seam::<C> { calls: PhantomData }, canvas, blend);
}

/// # Safety
/// As [`render_main_with`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_render_main_texture() {
    // SAFETY: the caller's contract is render_main_with's.
    unsafe { render_main_with::<Libobs>(false) }
}

/// # Safety
/// As [`render_main_with`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_render_main_texture_src_color_only() {
    // SAFETY: the caller's contract is render_main_with's.
    unsafe { render_main_with::<Libobs>(true) }
}

/// # Safety
/// As [`render_canvas_with`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_render_canvas_texture(canvas: *mut c_void) {
    // SAFETY: the caller's contract is render_canvas_with's.
    unsafe { render_canvas_with::<Libobs>(canvas, false) }
}

/// # Safety
/// As [`render_canvas_with`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_render_canvas_texture_src_color_only(canvas: *mut c_void) {
    // SAFETY: the caller's contract is render_canvas_with's.
    unsafe { render_canvas_with::<Libobs>(canvas, true) }
}
