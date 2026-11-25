//! C ABI for `libobs/obs-source-draw.c.h`: `obs_source_draw_set_color_matrix()`
//! and `obs_source_draw()` (`EXPORT` in `obs.h`, `rust-exports.d`).
//!
//! Each runs the safe core ([`crate::source_draw`]) on `Seam`, whose
//! libobs and graphics calls go through [`Calls`], so the parity test hands
//! both sides the same recording stand-ins; [`Libobs`] is libobs's own.
//! The matrix and the two ranges are read where the caller's pointers point
//! (16 and 3 floats, the bytes `gs_effect_set_matrix4()` and the 12-byte
//! `gs_effect_set_val()` copy) and handed on from an aligned copy; the
//! texture, effect and params pass as they came.
//!
//! No intentional differences from C.

use core::ffi::{CStr, c_char, c_int, c_void};
use core::marker::PhantomData;
use core::ptr::NonNull;

use crate::source_draw::{self as core_draw, Function, Graphics, Param};

/// `LOG_WARNING` (`util/base.h`).
pub const LOG_WARNING: c_int = 200;
/// `LOG_DEBUG`.
pub const LOG_DEBUG: c_int = 400;

/// `struct matrix4`: 16 floats, 16-byte aligned.
#[repr(C, align(16))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix4(pub [f32; 16]);

/// The libobs and graphics calls of the two functions, one C function each.
///
/// # Safety
/// Each method is the C function its doc names, called as the C calls it,
/// on the thread that holds the graphics context.
pub unsafe trait Calls {
    /// `gs_get_effect()`.
    fn get_effect() -> *mut c_void;
    /// `blog(level, format, ...)` with one or two string arguments, as the
    /// C's literal warning and `obs_ptr_valid()` log them.
    ///
    /// # Safety
    /// `format` is a static format taking `args` strings, each static and
    /// NUL-terminated; `args` is empty or one or two strings.
    unsafe fn blog(level: c_int, format: *const c_char, args: &[*const c_char]);
    /// `gs_effect_get_param_by_name(effect, name)`.
    ///
    /// # Safety
    /// `effect` is what [`Calls::get_effect`] gave; `name` is a static
    /// NUL-terminated string.
    unsafe fn param_by_name(effect: *mut c_void, name: *const c_char) -> *mut c_void;
    /// `gs_effect_set_matrix4(param, matrix)`.
    ///
    /// # Safety
    /// `param` is what [`Calls::param_by_name`] gave; `matrix` is readable.
    unsafe fn set_matrix4(param: *mut c_void, matrix: *const Matrix4);
    /// `gs_effect_set_val(param, val, size)`.
    ///
    /// # Safety
    /// As [`Calls::set_matrix4`], for `size` readable bytes at `val`.
    unsafe fn set_val(param: *mut c_void, val: *const c_void, size: usize);
    /// `gs_get_linear_srgb()`.
    fn get_linear_srgb() -> bool;
    /// `gs_framebuffer_srgb_enabled()`.
    fn framebuffer_srgb_enabled() -> bool;
    /// `gs_enable_framebuffer_srgb(enable)`.
    fn enable_framebuffer_srgb(enable: bool);
    /// `gs_effect_set_texture_srgb(param, texture)`.
    ///
    /// # Safety
    /// `param` is what [`Calls::param_by_name`] gave; `texture` is the
    /// caller's.
    unsafe fn set_texture_srgb(param: *mut c_void, texture: *mut c_void);
    /// `gs_effect_set_texture(param, texture)`.
    ///
    /// # Safety
    /// As [`Calls::set_texture_srgb`].
    unsafe fn set_texture(param: *mut c_void, texture: *mut c_void);
    /// `gs_matrix_push()`.
    fn matrix_push();
    /// `gs_matrix_translate3f(x, y, z)`.
    fn matrix_translate3f(x: f32, y: f32, z: f32);
    /// `gs_draw_sprite(texture, flip, width, height)`.
    ///
    /// # Safety
    /// `texture` is the caller's.
    unsafe fn draw_sprite(texture: *mut c_void, flip: u32, width: u32, height: u32);
    /// `gs_matrix_pop()`.
    fn matrix_pop();
}

/// [`Calls`] through libobs's own functions.
pub enum Libobs {}

unsafe extern "C" {
    fn gs_get_effect() -> *mut c_void;
    fn blog(level: c_int, format: *const c_char, ...);
    fn gs_effect_get_param_by_name(effect: *const c_void, name: *const c_char) -> *mut c_void;
    fn gs_effect_set_matrix4(param: *mut c_void, val: *const Matrix4);
    fn gs_effect_set_val(param: *mut c_void, val: *const c_void, size: usize);
    fn gs_get_linear_srgb() -> bool;
    fn gs_framebuffer_srgb_enabled() -> bool;
    fn gs_enable_framebuffer_srgb(enable: bool);
    fn gs_effect_set_texture_srgb(param: *mut c_void, val: *mut c_void);
    fn gs_effect_set_texture(param: *mut c_void, val: *mut c_void);
    fn gs_matrix_push();
    fn gs_matrix_translate3f(x: f32, y: f32, z: f32);
    fn gs_draw_sprite(tex: *mut c_void, flip: u32, width: u32, height: u32);
    fn gs_matrix_pop();
}

// SAFETY: each method is the one libobs function of its name, called with
// the caller's values.
unsafe impl Calls for Libobs {
    fn get_effect() -> *mut c_void {
        // SAFETY: takes nothing.
        unsafe { gs_get_effect() }
    }
    unsafe fn blog(level: c_int, format: *const c_char, args: &[*const c_char]) {
        // SAFETY: see the trait: a static format and its static strings.
        unsafe {
            match *args {
                [] => blog(level, format),
                [a] => blog(level, format, a),
                [a, b, ..] => blog(level, format, a, b),
            }
        }
    }
    unsafe fn param_by_name(effect: *mut c_void, name: *const c_char) -> *mut c_void {
        // SAFETY: see the trait.
        unsafe { gs_effect_get_param_by_name(effect, name) }
    }
    unsafe fn set_matrix4(param: *mut c_void, matrix: *const Matrix4) {
        // SAFETY: see the trait.
        unsafe { gs_effect_set_matrix4(param, matrix) }
    }
    unsafe fn set_val(param: *mut c_void, val: *const c_void, size: usize) {
        // SAFETY: see the trait.
        unsafe { gs_effect_set_val(param, val, size) }
    }
    fn get_linear_srgb() -> bool {
        // SAFETY: takes nothing.
        unsafe { gs_get_linear_srgb() }
    }
    fn framebuffer_srgb_enabled() -> bool {
        // SAFETY: takes nothing.
        unsafe { gs_framebuffer_srgb_enabled() }
    }
    fn enable_framebuffer_srgb(enable: bool) {
        // SAFETY: a value only.
        unsafe { gs_enable_framebuffer_srgb(enable) }
    }
    unsafe fn set_texture_srgb(param: *mut c_void, texture: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { gs_effect_set_texture_srgb(param, texture) }
    }
    unsafe fn set_texture(param: *mut c_void, texture: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { gs_effect_set_texture(param, texture) }
    }
    fn matrix_push() {
        // SAFETY: takes nothing.
        unsafe { gs_matrix_push() }
    }
    fn matrix_translate3f(x: f32, y: f32, z: f32) {
        // SAFETY: values only.
        unsafe { gs_matrix_translate3f(x, y, z) }
    }
    unsafe fn draw_sprite(texture: *mut c_void, flip: u32, width: u32, height: u32) {
        // SAFETY: see the trait.
        unsafe { gs_draw_sprite(texture, flip, width, height) }
    }
    fn matrix_pop() {
        // SAFETY: takes nothing.
        unsafe { gs_matrix_pop() }
    }
}

/// A `gs_effect_t *` or `gs_texture_t *` that is not NULL, passed back as
/// it came.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Object(NonNull<c_void>);

/// A `gs_eparam_t *` as the lookup gave it, NULL included.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Opaque(*mut c_void);

/// The core's [`Graphics`] over `C`.
///
/// Its methods are safe because the core passes back only the effect,
/// params and texture they gave or the caller handed in, all live under
/// the caller's contract; a seam is therefore made only by the `*_with`
/// functions below.
struct Seam<C: Calls> {
    calls: PhantomData<C>,
}

impl<C: Calls> Graphics for Seam<C> {
    type Effect = Object;
    type Param = Opaque;
    type Texture = Object;

    fn current_effect(&mut self) -> Option<Object> {
        NonNull::new(C::get_effect()).map(Object)
    }
    fn warn_no_effect(&mut self, function: Function) {
        // SAFETY: a static literal with no conversion, as the C passes it.
        unsafe { C::blog(LOG_WARNING, function.no_effect_warning().as_ptr(), &[]) }
    }
    fn log_null(&mut self, function: Function) {
        const FORMAT: &CStr = c"%s: Null '%s' parameter";
        // SAFETY: obs_object_valid()'s format with its two static strings.
        unsafe {
            C::blog(
                LOG_DEBUG,
                FORMAT.as_ptr(),
                &[function.name().as_ptr(), function.checked_param().as_ptr()],
            );
        }
    }
    fn param(&mut self, effect: Object, param: Param) -> Opaque {
        // SAFETY: the current effect as it came, and a static name.
        Opaque(unsafe { C::param_by_name(effect.0.as_ptr(), param.name().as_ptr()) })
    }
    fn set_matrix4(&mut self, param: Opaque, matrix: &[f32; 16]) {
        let matrix = Matrix4(*matrix);
        // SAFETY: the lookup's param as it came, and a live aligned matrix.
        unsafe { C::set_matrix4(param.0, &raw const matrix) }
    }
    fn set_range(&mut self, param: Opaque, range: &[f32; 3]) {
        // SAFETY: the lookup's param as it came, and 12 live bytes.
        unsafe { C::set_val(param.0, range.as_ptr().cast(), size_of::<[f32; 3]>()) }
    }
    fn linear_srgb(&mut self) -> bool {
        C::get_linear_srgb()
    }
    fn framebuffer_srgb_enabled(&mut self) -> bool {
        C::framebuffer_srgb_enabled()
    }
    fn enable_framebuffer_srgb(&mut self, enable: bool) {
        C::enable_framebuffer_srgb(enable);
    }
    fn set_texture_srgb(&mut self, param: Opaque, texture: Object) {
        // SAFETY: the lookup's param and the caller's texture as they came.
        unsafe { C::set_texture_srgb(param.0, texture.0.as_ptr()) }
    }
    fn set_texture(&mut self, param: Opaque, texture: Object) {
        // SAFETY: as in set_texture_srgb.
        unsafe { C::set_texture(param.0, texture.0.as_ptr()) }
    }
    fn matrix_push(&mut self) {
        C::matrix_push();
    }
    fn matrix_translate3f(&mut self, x: f32, y: f32, z: f32) {
        C::matrix_translate3f(x, y, z);
    }
    fn draw_sprite(&mut self, texture: Object, flip: u32, width: u32, height: u32) {
        // SAFETY: the caller's texture as it came.
        unsafe { C::draw_sprite(texture.0.as_ptr(), flip, width, height) }
    }
    fn matrix_pop(&mut self) {
        C::matrix_pop();
    }
}

/// [`obs_source_draw_set_color_matrix`] over `C`.
///
/// # Safety
/// Each pointer is NULL or points to readable memory of its C type
/// (`struct matrix4`, `struct vec3`); the caller holds the graphics
/// context.
pub unsafe fn set_color_matrix_with<C: Calls>(
    color_matrix: *const Matrix4,
    color_range_min: *const [f32; 3],
    color_range_max: *const [f32; 3],
) {
    // SAFETY: NULL or readable, per the caller's contract.
    let (color_matrix, color_range_min, color_range_max) = unsafe {
        (
            color_matrix.as_ref().map(|m| &m.0),
            color_range_min.as_ref(),
            color_range_max.as_ref(),
        )
    };
    core_draw::set_color_matrix(
        &mut Seam::<C> { calls: PhantomData },
        color_matrix,
        color_range_min,
        color_range_max,
    );
}

/// [`obs_source_draw`] over `C`.
///
/// # Safety
/// `texture` is NULL or a live texture; the caller holds the graphics
/// context.
pub unsafe fn draw_with<C: Calls>(
    texture: *mut c_void,
    x: c_int,
    y: c_int,
    cx: u32,
    cy: u32,
    flip: bool,
) {
    core_draw::draw(
        &mut Seam::<C> { calls: PhantomData },
        NonNull::new(texture).map(Object),
        x,
        y,
        cx,
        cy,
        flip,
    );
}

/// # Safety
/// As [`set_color_matrix_with`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_draw_set_color_matrix(
    color_matrix: *const Matrix4,
    color_range_min: *const [f32; 3],
    color_range_max: *const [f32; 3],
) {
    // SAFETY: the caller's contract is set_color_matrix_with's.
    unsafe { set_color_matrix_with::<Libobs>(color_matrix, color_range_min, color_range_max) }
}

/// # Safety
/// As [`draw_with`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_draw(
    texture: *mut c_void,
    x: c_int,
    y: c_int,
    cx: u32,
    cy: u32,
    flip: bool,
) {
    // SAFETY: the caller's contract is draw_with's.
    unsafe { draw_with::<Libobs>(texture, x, y, cx, cy, flip) }
}
