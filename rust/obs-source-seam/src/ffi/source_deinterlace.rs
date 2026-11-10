//! C ABI for the rest of `libobs/obs-source-deinterlace.c.h`, the
//! deinterlace of an async source: `deinterlace_process_last_frame()`,
//! `deinterlace_update_async_video()` and `deinterlace_render()`, declared
//! in `obs-internal.h` without `EXPORT` (`rust-hidden.d`), and
//! `obs_source_set_deinterlace_mode()` and
//! `obs_source_set_deinterlace_field_order()`, `EXPORT` through `obs.h`
//! (`rust-exports.d`).
//!
//! The functions are the safe core's (`obs_source::deinterlace`), run on
//! [`Seam`], which implements its `Source` over the deinterlace seam
//! (`libobs/obs-source-deinterlace-seam.h`, still C in `libobs.core-source`):
//! one accessor per field of `struct obs_source` and the core's video it
//! reads or writes, the lock and unlock of `async_mutex`, the queue calls,
//! the video time, `convert_video_space()` and a frame's `timestamp`,
//! `prev_frame` and reference; the mode and field order are read through
//! `obs_source_deinterlace_fields()`, as the getters read them. Neither
//! `struct obs_source` nor `struct obs_source_frame` is mirrored. The calls
//! into the rest of libobs go through [`Calls`]: the frame cache
//! (`remove_async_frame()`, `obs_source_release_frame()`), the async texture
//! upload (`set_async_texture_size()`, `update_async_textures()`), the
//! deinterlace texture size (`set_deinterlace_texture_size()`), the effect
//! load (`obs_load_effect()` on the seam's slots), the graphics context,
//! the SDR white level and the `gs_*` calls. [`Libobs`] calls them, and the
//! parity test hands in the oracle's recording stand-ins, as the C oracle
//! calls them. `obs_source_valid()` (a header inline) is `blog()` of the
//! same line for a NULL source.
//!
//! The locks are the C's: [`deinterlace_process_last_frame`] runs with
//! `async_mutex` held by its caller (`async_tick()`) and takes none;
//! [`deinterlace_update_async_video`] takes it only to read the slots and
//! take the previous frame, and uploads outside it; the enable of
//! [`obs_source_set_deinterlace_mode`] enters graphics, then takes it for
//! the release of the previous frame.
//!
//! No intentional differences from C.

use std::ffi::{CStr, c_char, c_int, c_long, c_void};
use std::marker::PhantomData;
use std::ptr;

use obs_source::deinterlace::{self as core, MAX_AV_PLANES, Source};
use obs_source::deinterlace_draw::Effect;

/// `LOG_DEBUG`.
const LOG_DEBUG: c_int = 400;

/// `struct obs_source_deinterlace_fields` (`obs-source-deinterlace-fields.c`).
#[repr(C)]
struct DeinterlaceFields {
    mode: *const c_int,
    top_first: *const bool,
}

/// `struct vec2`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2(pub [f32; 2]);

unsafe extern "C" {
    fn blog(level: c_int, format: *const c_char, ...);
    fn obs_source_deinterlace_fields(source: *const c_void) -> DeinterlaceFields;

    fn obs_source_deinterlace_seam_get_async_frames_num(source: *const c_void) -> usize;
    fn obs_source_deinterlace_seam_get_async_frame(
        source: *const c_void,
        idx: usize,
    ) -> *mut c_void;
    fn obs_source_deinterlace_seam_erase_async_frame_front(source: *mut c_void);
    fn obs_source_deinterlace_seam_async_mutex_lock(source: *mut c_void) -> c_int;
    fn obs_source_deinterlace_seam_async_mutex_unlock(source: *mut c_void) -> c_int;

    fn obs_source_deinterlace_seam_get_async_unbuffered(source: *const c_void) -> bool;
    fn obs_source_deinterlace_seam_get_last_sys_timestamp(source: *const c_void) -> u64;
    fn obs_source_deinterlace_seam_get_last_frame_ts(source: *const c_void) -> u64;
    fn obs_source_deinterlace_seam_set_last_frame_ts(source: *mut c_void, value: u64);
    fn obs_source_deinterlace_seam_get_deinterlace_offset(source: *const c_void) -> u64;
    fn obs_source_deinterlace_seam_set_deinterlace_offset(source: *mut c_void, value: u64);
    fn obs_source_deinterlace_seam_get_deinterlace_frame_ts(source: *const c_void) -> u64;
    fn obs_source_deinterlace_seam_set_deinterlace_frame_ts(source: *mut c_void, value: u64);
    fn obs_source_deinterlace_seam_get_deinterlace_half_duration(source: *const c_void) -> u32;
    fn obs_source_deinterlace_seam_set_deinterlace_half_duration(source: *mut c_void, value: u32);

    fn obs_source_deinterlace_seam_get_cur_async_frame(source: *const c_void) -> *mut c_void;
    fn obs_source_deinterlace_seam_set_cur_async_frame(source: *mut c_void, value: *mut c_void);
    fn obs_source_deinterlace_seam_get_prev_async_frame(source: *const c_void) -> *mut c_void;
    fn obs_source_deinterlace_seam_set_prev_async_frame(source: *mut c_void, value: *mut c_void);

    fn obs_source_deinterlace_seam_get_async_format(source: *const c_void) -> c_int;
    fn obs_source_deinterlace_seam_get_async_trc(source: *const c_void) -> u8;
    fn obs_source_deinterlace_seam_get_async_width(source: *const c_void) -> u32;
    fn obs_source_deinterlace_seam_get_async_height(source: *const c_void) -> u32;
    fn obs_source_deinterlace_seam_get_async_flip(source: *const c_void) -> bool;
    fn obs_source_deinterlace_seam_get_async_textures(source: *mut c_void) -> *mut *mut c_void;
    fn obs_source_deinterlace_seam_get_async_prev_textures(source: *mut c_void)
    -> *mut *mut c_void;
    fn obs_source_deinterlace_seam_get_async_texrender_slot(
        source: *mut c_void,
    ) -> *mut *mut c_void;
    fn obs_source_deinterlace_seam_get_async_prev_texrender_slot(
        source: *mut c_void,
    ) -> *mut *mut c_void;

    fn obs_source_deinterlace_seam_get_deinterlace_rendered(source: *const c_void) -> bool;
    fn obs_source_deinterlace_seam_set_deinterlace_rendered(source: *mut c_void, value: bool);
    fn obs_source_deinterlace_seam_get_deinterlace_effect(source: *const c_void) -> *mut c_void;
    fn obs_source_deinterlace_seam_set_deinterlace_effect(source: *mut c_void, value: *mut c_void);
    fn obs_source_deinterlace_seam_set_deinterlace_mode(source: *mut c_void, value: c_int);
    fn obs_source_deinterlace_seam_set_deinterlace_top_first(source: *mut c_void, value: bool);

    fn obs_source_deinterlace_seam_convert_video_space(format: c_int, trc: c_int) -> c_int;

    fn obs_source_deinterlace_seam_get_video_frame_interval_ns() -> u64;
    fn obs_source_deinterlace_seam_get_video_time() -> u64;
    fn obs_source_deinterlace_seam_get_discard_effect_slot() -> *mut *mut c_void;
    fn obs_source_deinterlace_seam_get_discard_2x_effect_slot() -> *mut *mut c_void;
    fn obs_source_deinterlace_seam_get_blend_effect_slot() -> *mut *mut c_void;
    fn obs_source_deinterlace_seam_get_blend_2x_effect_slot() -> *mut *mut c_void;
    fn obs_source_deinterlace_seam_get_linear_effect_slot() -> *mut *mut c_void;
    fn obs_source_deinterlace_seam_get_linear_2x_effect_slot() -> *mut *mut c_void;
    fn obs_source_deinterlace_seam_get_yadif_effect_slot() -> *mut *mut c_void;
    fn obs_source_deinterlace_seam_get_yadif_2x_effect_slot() -> *mut *mut c_void;

    fn obs_source_deinterlace_seam_get_frame_timestamp(frame: *const c_void) -> u64;
    fn obs_source_deinterlace_seam_get_frame_prev_frame(frame: *const c_void) -> bool;
    fn obs_source_deinterlace_seam_set_frame_prev_frame(frame: *mut c_void, value: bool);
    fn obs_source_deinterlace_seam_frame_addref(frame: *mut c_void) -> c_long;
}

/// The calls the deinterlace makes into the rest of libobs, one C function
/// each.
///
/// # Safety
/// Each is called with the pointers the C passes: the live source, a frame
/// it holds (queued, or in a slot), its texture slots, an effect slot of the
/// core, and the graphics context entered where the C has it.
pub unsafe trait Calls {
    /// `remove_async_frame(source, frame)`.
    ///
    /// # Safety
    /// `source` is live and `frame` one of its frames.
    unsafe fn remove_async_frame(source: *mut c_void, frame: *mut c_void);
    /// `set_async_texture_size(source, frame)`.
    ///
    /// # Safety
    /// As for [`Calls::remove_async_frame`], in the graphics context.
    unsafe fn set_async_texture_size(source: *mut c_void, frame: *const c_void) -> bool;
    /// `update_async_textures(source, frame, textures, texrender)`.
    ///
    /// # Safety
    /// As for [`Calls::set_async_texture_size`]; `textures` is the source's
    /// `MAX_AV_PLANES` texture slots.
    unsafe fn update_async_textures(
        source: *mut c_void,
        frame: *const c_void,
        textures: *mut *mut c_void,
        texrender: *mut c_void,
    ) -> bool;
    /// `obs_source_release_frame(source, frame)`.
    ///
    /// # Safety
    /// As for [`Calls::remove_async_frame`].
    unsafe fn release_frame(source: *mut c_void, frame: *mut c_void);
    /// `set_deinterlace_texture_size(source)`.
    ///
    /// # Safety
    /// `source` is live, in the graphics context.
    unsafe fn set_deinterlace_texture_size(source: *mut c_void);
    /// `obs_load_effect(effect, file)`.
    ///
    /// # Safety
    /// `effect` is one of the core's effect slots, `file` a NUL-terminated
    /// name, in the graphics context.
    unsafe fn load_effect(effect: *mut *mut c_void, file: *const c_char) -> *mut c_void;
    /// `obs_enter_graphics()`.
    ///
    /// # Safety
    /// The core is up.
    unsafe fn enter_graphics();
    /// `obs_leave_graphics()`.
    ///
    /// # Safety
    /// After [`Calls::enter_graphics`].
    unsafe fn leave_graphics();
    /// `obs_get_video_sdr_white_level()`.
    ///
    /// # Safety
    /// The core is up.
    unsafe fn sdr_white_level() -> f32;
    /// `gs_effect_get_param_by_name(effect, name)`.
    ///
    /// # Safety
    /// `effect` is NULL or a live effect; in the graphics context.
    unsafe fn effect_get_param_by_name(effect: *const c_void, name: *const c_char) -> *mut c_void;
    /// `gs_texrender_get_texture(texrender)`.
    ///
    /// # Safety
    /// `texrender` is a live texrender.
    unsafe fn texrender_get_texture(texrender: *const c_void) -> *mut c_void;
    /// `gs_get_linear_srgb()`.
    ///
    /// # Safety
    /// In the graphics context.
    unsafe fn get_linear_srgb() -> bool;
    /// `gs_get_color_space()`.
    ///
    /// # Safety
    /// In the graphics context.
    unsafe fn get_color_space() -> c_int;
    /// `gs_framebuffer_srgb_enabled()`.
    ///
    /// # Safety
    /// In the graphics context.
    unsafe fn framebuffer_srgb_enabled() -> bool;
    /// `gs_enable_framebuffer_srgb(enable)`.
    ///
    /// # Safety
    /// In the graphics context.
    unsafe fn enable_framebuffer_srgb(enable: bool);
    /// `gs_effect_set_texture_srgb(param, texture)`.
    ///
    /// # Safety
    /// `param` and `texture` are NULL or live; in the graphics context.
    unsafe fn effect_set_texture_srgb(param: *mut c_void, texture: *mut c_void);
    /// `gs_effect_set_texture(param, texture)`.
    ///
    /// # Safety
    /// As for [`Calls::effect_set_texture_srgb`].
    unsafe fn effect_set_texture(param: *mut c_void, texture: *mut c_void);
    /// `gs_effect_set_float(param, value)`.
    ///
    /// # Safety
    /// `param` is NULL or live; in the graphics context.
    unsafe fn effect_set_float(param: *mut c_void, value: f32);
    /// `gs_effect_set_int(param, value)`.
    ///
    /// # Safety
    /// As for [`Calls::effect_set_float`].
    unsafe fn effect_set_int(param: *mut c_void, value: c_int);
    /// `gs_effect_set_vec2(param, value)`.
    ///
    /// # Safety
    /// As for [`Calls::effect_set_float`]; `value` is a readable `struct
    /// vec2`.
    unsafe fn effect_set_vec2(param: *mut c_void, value: *const Vec2);
    /// `gs_effect_set_bool(param, value)`.
    ///
    /// # Safety
    /// As for [`Calls::effect_set_float`].
    unsafe fn effect_set_bool(param: *mut c_void, value: bool);
    /// `gs_effect_loop(effect, technique)`.
    ///
    /// # Safety
    /// `effect` is NULL or live, `technique` NUL-terminated; in the
    /// graphics context.
    unsafe fn effect_loop(effect: *mut c_void, technique: *const c_char) -> bool;
    /// `gs_draw_sprite(texture, flip, width, height)`.
    ///
    /// # Safety
    /// `texture` is NULL or live; in the graphics context.
    unsafe fn draw_sprite(texture: *mut c_void, flip: u32, width: u32, height: u32);
    /// `gs_texture_destroy(texture)`.
    ///
    /// # Safety
    /// `texture` is NULL or a live texture nothing else frees.
    unsafe fn texture_destroy(texture: *mut c_void);
    /// `gs_texrender_destroy(texrender)`.
    ///
    /// # Safety
    /// As for [`Calls::texture_destroy`].
    unsafe fn texrender_destroy(texrender: *mut c_void);
}

/// [`Calls`] through libobs's own functions.
pub enum Libobs {}

unsafe extern "C" {
    fn remove_async_frame(source: *mut c_void, frame: *mut c_void);
    fn set_async_texture_size(source: *mut c_void, frame: *const c_void) -> bool;
    fn update_async_textures(
        source: *mut c_void,
        frame: *const c_void,
        tex: *mut *mut c_void,
        texrender: *mut c_void,
    ) -> bool;
    fn obs_source_release_frame(source: *mut c_void, frame: *mut c_void);
    fn set_deinterlace_texture_size(source: *mut c_void);
    fn obs_load_effect(effect: *mut *mut c_void, file: *const c_char) -> *mut c_void;
    fn obs_enter_graphics();
    fn obs_leave_graphics();
    fn obs_get_video_sdr_white_level() -> f32;

    fn gs_effect_get_param_by_name(effect: *const c_void, name: *const c_char) -> *mut c_void;
    fn gs_texrender_get_texture(texrender: *const c_void) -> *mut c_void;
    fn gs_get_linear_srgb() -> bool;
    fn gs_get_color_space() -> c_int;
    fn gs_framebuffer_srgb_enabled() -> bool;
    fn gs_enable_framebuffer_srgb(enable: bool);
    fn gs_effect_set_texture_srgb(param: *mut c_void, val: *mut c_void);
    fn gs_effect_set_texture(param: *mut c_void, val: *mut c_void);
    fn gs_effect_set_float(param: *mut c_void, val: f32);
    fn gs_effect_set_int(param: *mut c_void, val: c_int);
    fn gs_effect_set_vec2(param: *mut c_void, val: *const Vec2);
    fn gs_effect_set_bool(param: *mut c_void, val: bool);
    fn gs_effect_loop(effect: *mut c_void, name: *const c_char) -> bool;
    fn gs_draw_sprite(tex: *mut c_void, flip: u32, width: u32, height: u32);
    fn gs_texture_destroy(tex: *mut c_void);
    fn gs_texrender_destroy(texrender: *mut c_void);
}

// SAFETY (every method): the C function of the same name, with the
// caller's pointers.
unsafe impl Calls for Libobs {
    unsafe fn remove_async_frame(source: *mut c_void, frame: *mut c_void) {
        // SAFETY: see the impl.
        unsafe { remove_async_frame(source, frame) }
    }
    unsafe fn set_async_texture_size(source: *mut c_void, frame: *const c_void) -> bool {
        // SAFETY: see the impl.
        unsafe { set_async_texture_size(source, frame) }
    }
    unsafe fn update_async_textures(
        source: *mut c_void,
        frame: *const c_void,
        textures: *mut *mut c_void,
        texrender: *mut c_void,
    ) -> bool {
        // SAFETY: see the impl.
        unsafe { update_async_textures(source, frame, textures, texrender) }
    }
    unsafe fn release_frame(source: *mut c_void, frame: *mut c_void) {
        // SAFETY: see the impl.
        unsafe { obs_source_release_frame(source, frame) }
    }
    unsafe fn set_deinterlace_texture_size(source: *mut c_void) {
        // SAFETY: see the impl.
        unsafe { set_deinterlace_texture_size(source) }
    }
    unsafe fn load_effect(effect: *mut *mut c_void, file: *const c_char) -> *mut c_void {
        // SAFETY: see the impl.
        unsafe { obs_load_effect(effect, file) }
    }
    unsafe fn enter_graphics() {
        // SAFETY: see the impl.
        unsafe { obs_enter_graphics() }
    }
    unsafe fn leave_graphics() {
        // SAFETY: see the impl.
        unsafe { obs_leave_graphics() }
    }
    unsafe fn sdr_white_level() -> f32 {
        // SAFETY: see the impl.
        unsafe { obs_get_video_sdr_white_level() }
    }
    unsafe fn effect_get_param_by_name(effect: *const c_void, name: *const c_char) -> *mut c_void {
        // SAFETY: see the impl.
        unsafe { gs_effect_get_param_by_name(effect, name) }
    }
    unsafe fn texrender_get_texture(texrender: *const c_void) -> *mut c_void {
        // SAFETY: see the impl.
        unsafe { gs_texrender_get_texture(texrender) }
    }
    unsafe fn get_linear_srgb() -> bool {
        // SAFETY: see the impl.
        unsafe { gs_get_linear_srgb() }
    }
    unsafe fn get_color_space() -> c_int {
        // SAFETY: see the impl.
        unsafe { gs_get_color_space() }
    }
    unsafe fn framebuffer_srgb_enabled() -> bool {
        // SAFETY: see the impl.
        unsafe { gs_framebuffer_srgb_enabled() }
    }
    unsafe fn enable_framebuffer_srgb(enable: bool) {
        // SAFETY: see the impl.
        unsafe { gs_enable_framebuffer_srgb(enable) }
    }
    unsafe fn effect_set_texture_srgb(param: *mut c_void, texture: *mut c_void) {
        // SAFETY: see the impl.
        unsafe { gs_effect_set_texture_srgb(param, texture) }
    }
    unsafe fn effect_set_texture(param: *mut c_void, texture: *mut c_void) {
        // SAFETY: see the impl.
        unsafe { gs_effect_set_texture(param, texture) }
    }
    unsafe fn effect_set_float(param: *mut c_void, value: f32) {
        // SAFETY: see the impl.
        unsafe { gs_effect_set_float(param, value) }
    }
    unsafe fn effect_set_int(param: *mut c_void, value: c_int) {
        // SAFETY: see the impl.
        unsafe { gs_effect_set_int(param, value) }
    }
    unsafe fn effect_set_vec2(param: *mut c_void, value: *const Vec2) {
        // SAFETY: see the impl.
        unsafe { gs_effect_set_vec2(param, value) }
    }
    unsafe fn effect_set_bool(param: *mut c_void, value: bool) {
        // SAFETY: see the impl.
        unsafe { gs_effect_set_bool(param, value) }
    }
    unsafe fn effect_loop(effect: *mut c_void, technique: *const c_char) -> bool {
        // SAFETY: see the impl.
        unsafe { gs_effect_loop(effect, technique) }
    }
    unsafe fn draw_sprite(texture: *mut c_void, flip: u32, width: u32, height: u32) {
        // SAFETY: see the impl.
        unsafe { gs_draw_sprite(texture, flip, width, height) }
    }
    unsafe fn texture_destroy(texture: *mut c_void) {
        // SAFETY: see the impl.
        unsafe { gs_texture_destroy(texture) }
    }
    unsafe fn texrender_destroy(texrender: *mut c_void) {
        // SAFETY: see the impl.
        unsafe { gs_texrender_destroy(texrender) }
    }
}

/// A pointer libobs handed out (a frame, texture, texrender, effect or
/// parameter), only passed back and compared here; never NULL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handle(*mut c_void);

fn handle(ptr: *mut c_void) -> Option<Handle> {
    (!ptr.is_null()).then_some(Handle(ptr))
}

fn handle_ptr(handle: Option<Handle>) -> *mut c_void {
    handle.map_or(ptr::null_mut(), |h| h.0)
}

/// A live `obs_source_t` a shim was handed, with the libobs calls `L` makes
/// for it.
pub struct Seam<L>(*mut c_void, PhantomData<L>);

impl<L> Seam<L> {
    fn new(source: *mut c_void) -> Self {
        Self(source, PhantomData)
    }

    fn fields(&self) -> DeinterlaceFields {
        // SAFETY: the live source; the adapter returns its fields'
        // addresses.
        unsafe { obs_source_deinterlace_fields(self.0) }
    }

    /// The slot `idx` of the `MAX_AV_PLANES` array at `base`.
    fn slot(base: *mut *mut c_void, idx: usize) -> *mut *mut c_void {
        debug_assert!(idx < MAX_AV_PLANES, "texture plane {idx}");
        // SAFETY: `base` is the start of a MAX_AV_PLANES array of the live
        // source, and `idx` is below it.
        unsafe { base.add(idx) }
    }
}

// SAFETY (every method): `self.0` is the live source the C caller passed,
// each frame one the seam read from its queue or slots, each texture,
// texrender, effect and parameter one libobs handed out, and each call is
// the one seam accessor, lock call or libobs function that names it, under
// the caller's locks and graphics context. A slot `idx` is below
// `MAX_AV_PLANES` ([`Seam::slot`] checks) and a queue `idx` below the
// count the core read under `async_mutex`.
impl<L: Calls> Source for Seam<L> {
    type Frame = Handle;
    type Texture = Handle;
    type Texrender = Handle;
    type Effect = Handle;
    type Param = Handle;

    fn valid(source: Option<&Self>, func: &'static CStr) -> bool {
        if source.is_some() {
            return true;
        }
        // SAFETY: obs_object_valid()'s line: the format and its two
        // string arguments are NUL terminated.
        unsafe {
            blog(
                LOG_DEBUG,
                c"%s: Null '%s' parameter".as_ptr(),
                func.as_ptr(),
                c"source".as_ptr(),
            );
        }
        false
    }

    fn frames_num(&self) -> usize {
        // SAFETY: see the impl.
        unsafe { obs_source_deinterlace_seam_get_async_frames_num(self.0) }
    }
    fn frame(&self, idx: usize) -> Handle {
        // SAFETY: see the impl.
        Handle(unsafe { obs_source_deinterlace_seam_get_async_frame(self.0, idx) })
    }
    fn erase_front(&self) {
        // SAFETY: see the impl.
        unsafe { obs_source_deinterlace_seam_erase_async_frame_front(self.0) }
    }
    fn lock(&self) {
        // SAFETY: see the impl. The C ignores the result too.
        unsafe { obs_source_deinterlace_seam_async_mutex_lock(self.0) };
    }
    fn unlock(&self) {
        // SAFETY: see the impl. The C ignores the result too.
        unsafe { obs_source_deinterlace_seam_async_mutex_unlock(self.0) };
    }

    fn async_unbuffered(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_deinterlace_seam_get_async_unbuffered(self.0) }
    }
    fn last_sys_timestamp(&self) -> u64 {
        // SAFETY: see the impl.
        unsafe { obs_source_deinterlace_seam_get_last_sys_timestamp(self.0) }
    }
    fn last_frame_ts(&self) -> u64 {
        // SAFETY: see the impl.
        unsafe { obs_source_deinterlace_seam_get_last_frame_ts(self.0) }
    }
    fn set_last_frame_ts(&self, value: u64) {
        // SAFETY: see the impl.
        unsafe { obs_source_deinterlace_seam_set_last_frame_ts(self.0, value) }
    }
    fn offset(&self) -> u64 {
        // SAFETY: see the impl.
        unsafe { obs_source_deinterlace_seam_get_deinterlace_offset(self.0) }
    }
    fn set_offset(&self, value: u64) {
        // SAFETY: see the impl.
        unsafe { obs_source_deinterlace_seam_set_deinterlace_offset(self.0, value) }
    }
    fn frame_ts(&self) -> u64 {
        // SAFETY: see the impl.
        unsafe { obs_source_deinterlace_seam_get_deinterlace_frame_ts(self.0) }
    }
    fn set_frame_ts(&self, value: u64) {
        // SAFETY: see the impl.
        unsafe { obs_source_deinterlace_seam_set_deinterlace_frame_ts(self.0, value) }
    }
    fn half_duration(&self) -> u32 {
        // SAFETY: see the impl.
        unsafe { obs_source_deinterlace_seam_get_deinterlace_half_duration(self.0) }
    }
    fn set_half_duration(&self, value: u32) {
        // SAFETY: see the impl.
        unsafe { obs_source_deinterlace_seam_set_deinterlace_half_duration(self.0, value) }
    }

    fn cur_frame(&self) -> Option<Handle> {
        // SAFETY: see the impl.
        handle(unsafe { obs_source_deinterlace_seam_get_cur_async_frame(self.0) })
    }
    fn set_cur_frame(&self, value: Option<Handle>) {
        // SAFETY: see the impl.
        unsafe { obs_source_deinterlace_seam_set_cur_async_frame(self.0, handle_ptr(value)) }
    }
    fn prev_frame(&self) -> Option<Handle> {
        // SAFETY: see the impl.
        handle(unsafe { obs_source_deinterlace_seam_get_prev_async_frame(self.0) })
    }
    fn set_prev_frame(&self, value: Option<Handle>) {
        // SAFETY: see the impl.
        unsafe { obs_source_deinterlace_seam_set_prev_async_frame(self.0, handle_ptr(value)) }
    }

    fn frame_timestamp(&self, frame: Handle) -> u64 {
        // SAFETY: see the impl.
        unsafe { obs_source_deinterlace_seam_get_frame_timestamp(frame.0) }
    }
    fn frame_prev_flag(&self, frame: Handle) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_deinterlace_seam_get_frame_prev_frame(frame.0) }
    }
    fn set_frame_prev_flag(&self, frame: Handle, value: bool) {
        // SAFETY: see the impl.
        unsafe { obs_source_deinterlace_seam_set_frame_prev_frame(frame.0, value) }
    }
    fn frame_addref(&self, frame: Handle) {
        // SAFETY: see the impl. The C ignores the count too.
        unsafe { obs_source_deinterlace_seam_frame_addref(frame.0) };
    }

    fn async_format(&self) -> i32 {
        // SAFETY: see the impl.
        unsafe { obs_source_deinterlace_seam_get_async_format(self.0) }
    }
    fn async_trc(&self) -> i32 {
        // SAFETY: see the impl.
        i32::from(unsafe { obs_source_deinterlace_seam_get_async_trc(self.0) })
    }
    fn async_width(&self) -> u32 {
        // SAFETY: see the impl.
        unsafe { obs_source_deinterlace_seam_get_async_width(self.0) }
    }
    fn async_height(&self) -> u32 {
        // SAFETY: see the impl.
        unsafe { obs_source_deinterlace_seam_get_async_height(self.0) }
    }
    fn async_flip(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_deinterlace_seam_get_async_flip(self.0) }
    }
    fn texture(&self, idx: usize) -> Option<Handle> {
        // SAFETY: see the impl.
        let slot = Self::slot(
            unsafe { obs_source_deinterlace_seam_get_async_textures(self.0) },
            idx,
        );
        // SAFETY: a slot of the live source.
        handle(unsafe { *slot })
    }
    fn set_texture(&self, idx: usize, value: Option<Handle>) {
        // SAFETY: see the impl.
        let slot = Self::slot(
            unsafe { obs_source_deinterlace_seam_get_async_textures(self.0) },
            idx,
        );
        // SAFETY: a slot of the live source.
        unsafe { *slot = handle_ptr(value) };
    }
    fn prev_texture(&self, idx: usize) -> Option<Handle> {
        let slot = Self::slot(
            // SAFETY: see the impl.
            unsafe { obs_source_deinterlace_seam_get_async_prev_textures(self.0) },
            idx,
        );
        // SAFETY: a slot of the live source.
        handle(unsafe { *slot })
    }
    fn set_prev_texture(&self, idx: usize, value: Option<Handle>) {
        let slot = Self::slot(
            // SAFETY: see the impl.
            unsafe { obs_source_deinterlace_seam_get_async_prev_textures(self.0) },
            idx,
        );
        // SAFETY: a slot of the live source.
        unsafe { *slot = handle_ptr(value) };
    }
    fn texrender(&self) -> Option<Handle> {
        // SAFETY: see the impl; the slot is the field's own storage.
        handle(unsafe { *obs_source_deinterlace_seam_get_async_texrender_slot(self.0) })
    }
    fn set_texrender(&self, value: Option<Handle>) {
        // SAFETY: as for `texrender`.
        unsafe {
            *obs_source_deinterlace_seam_get_async_texrender_slot(self.0) = handle_ptr(value)
        };
    }
    fn prev_texrender(&self) -> Option<Handle> {
        // SAFETY: as for `texrender`.
        handle(unsafe { *obs_source_deinterlace_seam_get_async_prev_texrender_slot(self.0) })
    }
    fn set_prev_texrender(&self, value: Option<Handle>) {
        // SAFETY: as for `texrender`.
        unsafe {
            *obs_source_deinterlace_seam_get_async_prev_texrender_slot(self.0) = handle_ptr(value);
        };
    }

    fn rendered(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_deinterlace_seam_get_deinterlace_rendered(self.0) }
    }
    fn set_rendered(&self, value: bool) {
        // SAFETY: see the impl.
        unsafe { obs_source_deinterlace_seam_set_deinterlace_rendered(self.0, value) }
    }
    fn effect(&self) -> Option<Handle> {
        // SAFETY: see the impl.
        handle(unsafe { obs_source_deinterlace_seam_get_deinterlace_effect(self.0) })
    }
    fn set_effect(&self, value: Option<Handle>) {
        // SAFETY: see the impl.
        unsafe { obs_source_deinterlace_seam_set_deinterlace_effect(self.0, handle_ptr(value)) }
    }
    fn mode(&self) -> i32 {
        // SAFETY: the address of the live source's `deinterlace_mode`.
        unsafe { *self.fields().mode }
    }
    fn set_mode(&self, value: i32) {
        // SAFETY: see the impl.
        unsafe { obs_source_deinterlace_seam_set_deinterlace_mode(self.0, value) }
    }
    fn top_first(&self) -> bool {
        // SAFETY: the address of the live source's `deinterlace_top_first`.
        unsafe { *self.fields().top_first }
    }
    fn set_top_first(&self, value: bool) {
        // SAFETY: see the impl.
        unsafe { obs_source_deinterlace_seam_set_deinterlace_top_first(self.0, value) }
    }

    fn convert_video_space(&self, format: i32, trc: i32) -> i32 {
        // SAFETY: a value call.
        unsafe { obs_source_deinterlace_seam_convert_video_space(format, trc) }
    }
    fn frame_interval_ns(&self) -> u64 {
        // SAFETY: the core is up.
        unsafe { obs_source_deinterlace_seam_get_video_frame_interval_ns() }
    }
    fn video_time(&self) -> u64 {
        // SAFETY: the core is up.
        unsafe { obs_source_deinterlace_seam_get_video_time() }
    }
    fn load_effect(&self, effect: Effect) -> Option<Handle> {
        // SAFETY: the core is up; each slot accessor gives the address of
        // one of its effect fields.
        let (slot, file) = unsafe {
            match effect {
                Effect::Discard => (
                    obs_source_deinterlace_seam_get_discard_effect_slot(),
                    c"deinterlace_discard.effect",
                ),
                Effect::Discard2x => (
                    obs_source_deinterlace_seam_get_discard_2x_effect_slot(),
                    c"deinterlace_discard_2x.effect",
                ),
                Effect::Blend => (
                    obs_source_deinterlace_seam_get_blend_effect_slot(),
                    c"deinterlace_blend.effect",
                ),
                Effect::Blend2x => (
                    obs_source_deinterlace_seam_get_blend_2x_effect_slot(),
                    c"deinterlace_blend_2x.effect",
                ),
                Effect::Linear => (
                    obs_source_deinterlace_seam_get_linear_effect_slot(),
                    c"deinterlace_linear.effect",
                ),
                Effect::Linear2x => (
                    obs_source_deinterlace_seam_get_linear_2x_effect_slot(),
                    c"deinterlace_linear_2x.effect",
                ),
                Effect::Yadif => (
                    obs_source_deinterlace_seam_get_yadif_effect_slot(),
                    c"deinterlace_yadif.effect",
                ),
                Effect::Yadif2x => (
                    obs_source_deinterlace_seam_get_yadif_2x_effect_slot(),
                    c"deinterlace_yadif_2x.effect",
                ),
            }
        };
        // SAFETY: see the impl.
        handle(unsafe { L::load_effect(slot, file.as_ptr()) })
    }

    fn remove_async_frame(&self, frame: Handle) {
        // SAFETY: see the impl.
        unsafe { L::remove_async_frame(self.0, frame.0) }
    }
    fn set_async_texture_size(&self, frame: Handle) -> bool {
        // SAFETY: see the impl.
        unsafe { L::set_async_texture_size(self.0, frame.0) }
    }
    fn update_prev_textures(&self, frame: Handle, prev_texrender: Option<Handle>) -> bool {
        // SAFETY: see the impl; the previous textures' slots are the
        // field's own storage, as the C passes them.
        unsafe {
            L::update_async_textures(
                self.0,
                frame.0,
                obs_source_deinterlace_seam_get_async_prev_textures(self.0),
                handle_ptr(prev_texrender),
            )
        }
    }
    fn release_frame(&self, frame: Handle) {
        // SAFETY: see the impl.
        unsafe { L::release_frame(self.0, frame.0) }
    }
    fn set_deinterlace_texture_size(&self) {
        // SAFETY: see the impl.
        unsafe { L::set_deinterlace_texture_size(self.0) }
    }

    fn enter_graphics(&self) {
        // SAFETY: see the impl.
        unsafe { L::enter_graphics() }
    }
    fn leave_graphics(&self) {
        // SAFETY: see the impl.
        unsafe { L::leave_graphics() }
    }
    fn sdr_white_level(&self) -> f32 {
        // SAFETY: see the impl.
        unsafe { L::sdr_white_level() }
    }

    fn param_by_name(&self, effect: Option<Handle>, name: &'static CStr) -> Option<Handle> {
        // SAFETY: see the impl.
        handle(unsafe { L::effect_get_param_by_name(handle_ptr(effect), name.as_ptr()) })
    }
    fn texrender_texture(&self, texrender: Handle) -> Option<Handle> {
        // SAFETY: see the impl.
        handle(unsafe { L::texrender_get_texture(texrender.0) })
    }
    fn linear_srgb(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { L::get_linear_srgb() }
    }
    fn color_space(&self) -> i32 {
        // SAFETY: see the impl.
        unsafe { L::get_color_space() }
    }
    fn framebuffer_srgb_enabled(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { L::framebuffer_srgb_enabled() }
    }
    fn enable_framebuffer_srgb(&self, enable: bool) {
        // SAFETY: see the impl.
        unsafe { L::enable_framebuffer_srgb(enable) }
    }
    fn set_param_texture_srgb(&self, param: Option<Handle>, texture: Option<Handle>) {
        // SAFETY: see the impl.
        unsafe { L::effect_set_texture_srgb(handle_ptr(param), handle_ptr(texture)) }
    }
    fn set_param_texture(&self, param: Option<Handle>, texture: Option<Handle>) {
        // SAFETY: see the impl.
        unsafe { L::effect_set_texture(handle_ptr(param), handle_ptr(texture)) }
    }
    fn set_param_float(&self, param: Option<Handle>, value: f32) {
        // SAFETY: see the impl.
        unsafe { L::effect_set_float(handle_ptr(param), value) }
    }
    fn set_param_int(&self, param: Option<Handle>, value: i32) {
        // SAFETY: see the impl.
        unsafe { L::effect_set_int(handle_ptr(param), value) }
    }
    fn set_param_vec2(&self, param: Option<Handle>, value: [f32; 2]) {
        let value = Vec2(value);
        // SAFETY: see the impl; `value` lives across the call.
        unsafe { L::effect_set_vec2(handle_ptr(param), &raw const value) }
    }
    fn set_param_bool(&self, param: Option<Handle>, value: bool) {
        // SAFETY: see the impl.
        unsafe { L::effect_set_bool(handle_ptr(param), value) }
    }
    fn effect_loop(&self, effect: Option<Handle>, technique: &'static CStr) -> bool {
        // SAFETY: see the impl.
        unsafe { L::effect_loop(handle_ptr(effect), technique.as_ptr()) }
    }
    fn draw_sprite(&self, flip: u32, width: u32, height: u32) {
        // SAFETY: see the impl; the C passes no texture.
        unsafe { L::draw_sprite(ptr::null_mut(), flip, width, height) }
    }
    fn texture_destroy(&self, texture: Option<Handle>) {
        // SAFETY: see the impl.
        unsafe { L::texture_destroy(handle_ptr(texture)) }
    }
    fn texrender_destroy(&self, texrender: Option<Handle>) {
        // SAFETY: see the impl.
        unsafe { L::texrender_destroy(handle_ptr(texrender)) }
    }
}

fn seam<L>(source: *mut c_void) -> Option<Seam<L>> {
    (!source.is_null()).then(|| Seam::new(source))
}

/// `deinterlace_process_last_frame()` with the libobs calls of `L`.
///
/// # Safety
/// `source` is a live async source whose `async_mutex` the caller holds.
pub unsafe fn process_last_frame<L: Calls>(source: *mut c_void, sys_time: u64) {
    core::process_last_frame(&Seam::<L>::new(source), sys_time);
}

/// `deinterlace_update_async_video()` with the libobs calls of `L`.
///
/// # Safety
/// `source` is a live async source; the graphics context is entered and
/// `async_mutex` not held.
pub unsafe fn update_async_video<L: Calls>(source: *mut c_void) {
    core::update_async_video(&Seam::<L>::new(source));
}

/// `deinterlace_render()` with the libobs calls of `L`.
///
/// # Safety
/// `source` is a live async source; the graphics context is entered.
pub unsafe fn render<L: Calls>(source: *mut c_void) {
    core::render(&Seam::<L>::new(source));
}

/// `obs_source_set_deinterlace_mode()` with the libobs calls of `L`.
///
/// # Safety
/// `source` is NULL or a live source; neither the graphics context nor
/// `async_mutex` is held.
pub unsafe fn set_mode<L: Calls>(source: *mut c_void, mode: c_int) {
    core::set_mode(seam::<L>(source).as_ref(), mode);
}

/// `obs_source_set_deinterlace_field_order()` with the libobs calls of `L`.
///
/// # Safety
/// `source` is NULL or a live source.
pub unsafe fn set_field_order<L: Calls>(source: *mut c_void, field_order: c_int) {
    core::set_field_order(seam::<L>(source).as_ref(), field_order);
}

/// `void deinterlace_process_last_frame(obs_source_t *source, uint64_t
/// sys_time)`.
///
/// # Safety
/// As for [`process_last_frame`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deinterlace_process_last_frame(source: *mut c_void, sys_time: u64) {
    // SAFETY: the caller's contract.
    unsafe { process_last_frame::<Libobs>(source, sys_time) }
}

/// `void deinterlace_update_async_video(obs_source_t *source)`.
///
/// # Safety
/// As for [`update_async_video`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deinterlace_update_async_video(source: *mut c_void) {
    // SAFETY: the caller's contract.
    unsafe { update_async_video::<Libobs>(source) }
}

/// `void deinterlace_render(obs_source_t *s)`.
///
/// # Safety
/// As for [`render`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deinterlace_render(source: *mut c_void) {
    // SAFETY: the caller's contract.
    unsafe { render::<Libobs>(source) }
}

/// `void obs_source_set_deinterlace_mode(obs_source_t *source, enum
/// obs_deinterlace_mode mode)`.
///
/// # Safety
/// As for [`set_mode`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_set_deinterlace_mode(source: *mut c_void, mode: c_int) {
    // SAFETY: the caller's contract.
    unsafe { set_mode::<Libobs>(source, mode) }
}

/// `void obs_source_set_deinterlace_field_order(obs_source_t *source, enum
/// obs_deinterlace_field_order field_order)`.
///
/// # Safety
/// As for [`set_field_order`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_set_deinterlace_field_order(
    source: *mut c_void,
    field_order: c_int,
) {
    // SAFETY: the caller's contract.
    unsafe { set_field_order::<Libobs>(source, field_order) }
}
