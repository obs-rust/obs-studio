//! C ABI for `libobs/obs-display.c.h`, the display: `obs_display_create()`,
//! `obs_display_destroy()`, `obs_display_resize()`,
//! `obs_display_update_color_space()`, `obs_display_add_draw_callback()` and
//! `obs_display_remove_draw_callback()` (`EXPORT` in `obs.h`,
//! `rust-exports.d`), and `obs_display_init()`, `obs_display_free()`
//! (`obs-internal.h`) and `render_display()` (called from
//! `render_displays()` in `obs-video.c.h`), hidden (`rust-hidden.d`).
//!
//! Each runs the safe core ([`crate::display`]) on `Seam`, which
//! implements its [`Host`] over the display seam (`libobs/obs-display-seam.h`,
//! still C in `libobs.core-source`) and the size fields
//! (`obs-display-size-fields.c.h`): one seam call per field, lock, list link
//! and container call. `struct obs_display` and `struct obs_core` are never
//! mirrored, so the fields stay in C and ThreadSanitizer keeps seeing their
//! reads and writes. The display is allocated with libobs's `bmalloc()`,
//! zeroed as `bzalloc()` does, at the seam's `sizeof(struct obs_display)`.
//!
//! The calls into the rest of libobs and the graphics subsystem
//! (`blog()`, the two host answers, `obs_enter_graphics()` and
//! `obs_leave_graphics()`, and the `gs_*` calls) go through [`Calls`], so the
//! parity test hands both sides the same recording stand-ins. [`Libobs`] is
//! libobs's own. A draw callback is called through the caller's pointer,
//! with `draw_callbacks_mutex` held and no Rust borrow across the call.
//!
//! One intentional difference: a draw callback added with a NULL function is
//! skipped, where the C calls through NULL and crashes.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::marker::PhantomData;
use std::ptr::{self, NonNull};

use obs_util::ffi::darray::{bfree, bmalloc};

use crate::display::{self as core, Host};

/// `LOG_ERROR` in `util/base.h`.
const LOG_ERROR: c_int = 100;

/// A draw callback's function: `void (*draw)(void *param, uint32_t cx,
/// uint32_t cy)`.
pub type DrawFn = unsafe extern "C" fn(param: *mut c_void, cx: u32, cy: u32);

/// `struct draw_callback` (`obs-internal.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
pub struct draw_callback {
    /// `draw`.
    pub draw: Option<DrawFn>,
    /// `param`.
    pub param: *mut c_void,
}

/// `struct vec4` (`graphics/vec4.h`): a union with `__m128`, so 16-aligned.
#[repr(C, align(16))]
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(non_camel_case_types)]
pub struct vec4 {
    /// `x`.
    pub x: f32,
    /// `y`.
    pub y: f32,
    /// `z`.
    pub z: f32,
    /// `w`.
    pub w: f32,
}

impl From<core::Vec4> for vec4 {
    fn from(v: core::Vec4) -> Self {
        Self {
            x: v.x,
            y: v.y,
            z: v.z,
            w: v.w,
        }
    }
}

/// `struct obs_display_size_fields` (`obs-display-size-fields.c.h`).
#[repr(C)]
struct SizeFields {
    cx: *mut u32,
    cy: *mut u32,
}

unsafe extern "C" {
    fn obs_display_size_fields(display: *mut c_void) -> SizeFields;
    fn obs_display_size_lock(display: *mut c_void);
    fn obs_display_size_unlock(display: *mut c_void);

    fn obs_display_seam_init_mutex_values(display: *mut c_void);
    fn obs_display_seam_callbacks_mutex_init(display: *mut c_void) -> c_int;
    fn obs_display_seam_info_mutex_init(display: *mut c_void) -> c_int;
    fn obs_display_seam_callbacks_mutex_destroy(display: *mut c_void);
    fn obs_display_seam_info_mutex_destroy(display: *mut c_void);
    fn obs_display_seam_lock_callbacks(display: *mut c_void) -> c_int;
    fn obs_display_seam_unlock_callbacks(display: *mut c_void) -> c_int;

    fn obs_display_seam_get_use_clear_workaround(display: *const c_void) -> bool;
    fn obs_display_seam_set_use_clear_workaround(display: *mut c_void, value: bool);
    fn obs_display_seam_get_swap(display: *const c_void) -> *mut c_void;
    fn obs_display_seam_set_swap(display: *mut c_void, value: *mut c_void);
    fn obs_display_seam_get_next_cx(display: *const c_void) -> u32;
    fn obs_display_seam_set_next_cx(display: *mut c_void, value: u32);
    fn obs_display_seam_get_next_cy(display: *const c_void) -> u32;
    fn obs_display_seam_set_next_cy(display: *mut c_void, value: u32);
    fn obs_display_seam_get_update_color_space(display: *const c_void) -> bool;
    fn obs_display_seam_set_update_color_space(display: *mut c_void, value: bool);
    fn obs_display_seam_get_enabled(display: *const c_void) -> bool;
    fn obs_display_seam_set_enabled(display: *mut c_void, value: bool);
    fn obs_display_seam_get_background_color(display: *const c_void) -> u32;
    fn obs_display_seam_set_background_color(display: *mut c_void, value: u32);

    fn obs_display_seam_callbacks_push(display: *mut c_void, callback: *const draw_callback);
    fn obs_display_seam_callbacks_erase_item(display: *mut c_void, callback: *const draw_callback);
    fn obs_display_seam_callbacks_num(display: *const c_void) -> usize;
    fn obs_display_seam_callbacks_get(display: *mut c_void, idx: usize) -> *mut draw_callback;
    fn obs_display_seam_callbacks_free(display: *mut c_void);

    fn obs_display_seam_lock_displays() -> c_int;
    fn obs_display_seam_unlock_displays() -> c_int;
    fn obs_display_seam_get_first_display() -> *mut c_void;
    fn obs_display_seam_set_first_display(value: *mut c_void);
    fn obs_display_seam_first_display_ref() -> *mut *mut c_void;
    fn obs_display_seam_get_next(display: *const c_void) -> *mut c_void;
    fn obs_display_seam_set_next(display: *mut c_void, value: *mut c_void);
    fn obs_display_seam_next_ref(display: *mut c_void) -> *mut *mut c_void;
    fn obs_display_seam_get_prev_next(display: *const c_void) -> *mut *mut c_void;
    fn obs_display_seam_set_prev_next(display: *mut c_void, value: *mut *mut c_void);

    fn obs_display_seam_get_graphics() -> *mut c_void;
    fn obs_display_seam_get_solid_effect() -> *mut c_void;

    fn obs_display_seam_size() -> usize;
    fn obs_display_seam_get_init_cx(graphics_data: *const c_void) -> u32;
    fn obs_display_seam_get_init_cy(graphics_data: *const c_void) -> u32;
}

/// The calls the display makes into the rest of libobs and the graphics
/// subsystem, one C function each.
///
/// # Safety
/// Each method is the C function its doc names, called as the C calls it:
/// the graphics calls on the thread that holds the graphics context.
pub unsafe trait Calls {
    /// `blog(LOG_ERROR, message)`, the message as the format.
    fn log_error(message: &'static CStr);
    /// `obs_host_display_clear_workaround()`.
    fn host_clear_workaround() -> bool;
    /// `obs_host_display_clear_color_always_srgb()`.
    fn host_clear_color_always_srgb() -> bool;
    /// `obs_enter_graphics()`.
    fn enter_graphics();
    /// `obs_leave_graphics()`.
    fn leave_graphics();
    /// `gs_enter_context(graphics)`.
    ///
    /// # Safety
    /// `graphics` is the core's graphics or NULL.
    unsafe fn enter_context(graphics: *mut c_void);
    /// `gs_leave_context()`.
    fn leave_context();
    /// `gs_swapchain_create(data)`.
    ///
    /// # Safety
    /// `data` is a live `struct gs_init_data`.
    unsafe fn swapchain_create(data: *const c_void) -> *mut c_void;
    /// `gs_swapchain_destroy(swap)`.
    ///
    /// # Safety
    /// `swap` is a live swap chain.
    unsafe fn swapchain_destroy(swap: *mut c_void);
    /// `gs_load_swapchain(swap)`.
    ///
    /// # Safety
    /// `swap` is a live swap chain or NULL.
    unsafe fn load_swapchain(swap: *mut c_void);
    /// `gs_resize(cx, cy)`.
    fn resize(cx: u32, cy: u32);
    /// `gs_update_color_space()`.
    fn update_color_space();
    /// `gs_is_present_ready()`.
    fn is_present_ready() -> bool;
    /// `gs_begin_scene()`.
    fn begin_scene();
    /// `(int)gs_get_color_space()`.
    fn color_space() -> c_int;
    /// `gs_clear(flags, color, depth, stencil)`.
    fn clear(flags: u32, color: &vec4, depth: f32, stencil: u8);
    /// `gs_enable_depth_test(enable)`.
    fn enable_depth_test(enable: bool);
    /// `gs_set_cull_mode(mode)`.
    fn set_cull_mode(mode: c_int);
    /// `gs_ortho(left, right, top, bottom, znear, zfar)`.
    fn ortho(left: f32, right: f32, top: f32, bottom: f32, znear: f32, zfar: f32);
    /// `gs_set_viewport(x, y, width, height)`.
    fn set_viewport(x: c_int, y: c_int, width: c_int, height: c_int);
    /// `gs_effect_get_param_by_name(effect, name)`.
    ///
    /// # Safety
    /// `effect` is the core's solid effect or NULL.
    unsafe fn param_by_name(effect: *mut c_void, name: *const c_char) -> *mut c_void;
    /// `gs_effect_set_vec4(param, value)`.
    ///
    /// # Safety
    /// `param` is what [`Calls::param_by_name`] gave.
    unsafe fn effect_set_vec4(param: *mut c_void, value: &vec4);
    /// `gs_effect_loop(effect, technique)`.
    ///
    /// # Safety
    /// As [`Calls::param_by_name`].
    unsafe fn effect_loop(effect: *mut c_void, technique: *const c_char) -> bool;
    /// `gs_draw_sprite(NULL, 0, cx, cy)`.
    fn draw_sprite(cx: u32, cy: u32);
    /// `gs_end_scene()`.
    fn end_scene();
    /// `gs_present()`.
    fn present();
}

/// [`Calls`] through libobs's own functions.
pub enum Libobs {}

unsafe extern "C" {
    fn blog(level: c_int, format: *const c_char, ...);
    fn obs_host_display_clear_workaround() -> bool;
    fn obs_host_display_clear_color_always_srgb() -> bool;
    fn obs_enter_graphics();
    fn obs_leave_graphics();
    fn gs_enter_context(graphics: *mut c_void);
    fn gs_leave_context();
    fn gs_swapchain_create(data: *const c_void) -> *mut c_void;
    fn gs_swapchain_destroy(swap: *mut c_void);
    fn gs_load_swapchain(swap: *mut c_void);
    fn gs_resize(cx: u32, cy: u32);
    fn gs_update_color_space();
    fn gs_is_present_ready() -> bool;
    fn gs_begin_scene();
    fn gs_get_color_space() -> c_int;
    fn gs_clear(flags: u32, color: *const vec4, depth: f32, stencil: u8);
    fn gs_enable_depth_test(enable: bool);
    fn gs_set_cull_mode(mode: c_int);
    fn gs_ortho(left: f32, right: f32, top: f32, bottom: f32, znear: f32, zfar: f32);
    fn gs_set_viewport(x: c_int, y: c_int, width: c_int, height: c_int);
    fn gs_effect_get_param_by_name(effect: *const c_void, name: *const c_char) -> *mut c_void;
    fn gs_effect_set_vec4(param: *mut c_void, value: *const vec4);
    fn gs_effect_loop(effect: *mut c_void, technique: *const c_char) -> bool;
    fn gs_draw_sprite(tex: *mut c_void, flip: u32, width: u32, height: u32);
    fn gs_end_scene();
    fn gs_present();
}

// SAFETY: each method is the one libobs function of its name, called with
// the caller's values.
unsafe impl Calls for Libobs {
    fn log_error(message: &'static CStr) {
        // SAFETY: a static string with no conversions, as the C's format.
        unsafe { blog(LOG_ERROR, message.as_ptr()) }
    }
    fn host_clear_workaround() -> bool {
        // SAFETY: takes nothing.
        unsafe { obs_host_display_clear_workaround() }
    }
    fn host_clear_color_always_srgb() -> bool {
        // SAFETY: takes nothing.
        unsafe { obs_host_display_clear_color_always_srgb() }
    }
    fn enter_graphics() {
        // SAFETY: takes nothing.
        unsafe { obs_enter_graphics() }
    }
    fn leave_graphics() {
        // SAFETY: takes nothing.
        unsafe { obs_leave_graphics() }
    }
    unsafe fn enter_context(graphics: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { gs_enter_context(graphics) }
    }
    fn leave_context() {
        // SAFETY: takes nothing.
        unsafe { gs_leave_context() }
    }
    unsafe fn swapchain_create(data: *const c_void) -> *mut c_void {
        // SAFETY: see the trait.
        unsafe { gs_swapchain_create(data) }
    }
    unsafe fn swapchain_destroy(swap: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { gs_swapchain_destroy(swap) }
    }
    unsafe fn load_swapchain(swap: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { gs_load_swapchain(swap) }
    }
    fn resize(cx: u32, cy: u32) {
        // SAFETY: values only.
        unsafe { gs_resize(cx, cy) }
    }
    fn update_color_space() {
        // SAFETY: takes nothing.
        unsafe { gs_update_color_space() }
    }
    fn is_present_ready() -> bool {
        // SAFETY: takes nothing.
        unsafe { gs_is_present_ready() }
    }
    fn begin_scene() {
        // SAFETY: takes nothing.
        unsafe { gs_begin_scene() }
    }
    fn color_space() -> c_int {
        // SAFETY: takes nothing.
        unsafe { gs_get_color_space() }
    }
    fn clear(flags: u32, color: &vec4, depth: f32, stencil: u8) {
        // SAFETY: a live vec4 for the call.
        unsafe { gs_clear(flags, color, depth, stencil) }
    }
    fn enable_depth_test(enable: bool) {
        // SAFETY: a value only.
        unsafe { gs_enable_depth_test(enable) }
    }
    fn set_cull_mode(mode: c_int) {
        // SAFETY: a value only.
        unsafe { gs_set_cull_mode(mode) }
    }
    fn ortho(left: f32, right: f32, top: f32, bottom: f32, znear: f32, zfar: f32) {
        // SAFETY: values only.
        unsafe { gs_ortho(left, right, top, bottom, znear, zfar) }
    }
    fn set_viewport(x: c_int, y: c_int, width: c_int, height: c_int) {
        // SAFETY: values only.
        unsafe { gs_set_viewport(x, y, width, height) }
    }
    unsafe fn param_by_name(effect: *mut c_void, name: *const c_char) -> *mut c_void {
        // SAFETY: see the trait; the name is a static string.
        unsafe { gs_effect_get_param_by_name(effect, name) }
    }
    unsafe fn effect_set_vec4(param: *mut c_void, value: &vec4) {
        // SAFETY: see the trait.
        unsafe { gs_effect_set_vec4(param, value) }
    }
    unsafe fn effect_loop(effect: *mut c_void, technique: *const c_char) -> bool {
        // SAFETY: see the trait; the technique is a static string.
        unsafe { gs_effect_loop(effect, technique) }
    }
    fn draw_sprite(cx: u32, cy: u32) {
        // SAFETY: NULL draws the effect's own texture, as the C passes it.
        unsafe { gs_draw_sprite(ptr::null_mut(), 0, cx, cy) }
    }
    fn end_scene() {
        // SAFETY: takes nothing.
        unsafe { gs_end_scene() }
    }
    fn present() {
        // SAFETY: takes nothing.
        unsafe { gs_present() }
    }
}

/// A `graphics_t *`, `gs_effect_t *` or `gs_eparam_t *` the core holds or a
/// lookup gave, passed back to the graphics subsystem as it came.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Opaque(*mut c_void);

/// A non-NULL `obs_display_t *`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Display(NonNull<c_void>);

impl Display {
    fn ptr(self) -> *mut c_void {
        self.0.as_ptr()
    }
}

fn display_or_null(display: Option<Display>) -> *mut c_void {
    display.map_or(ptr::null_mut(), Display::ptr)
}

/// The core's [`Host`] over the display seam and `C`.
///
/// Its methods are safe because the core calls them only on displays it
/// got from the caller or from `alloc()`, and frees each once; a seam is
/// therefore made only by the `*_with` functions below, under their
/// callers' contracts, and is private to the crate.
pub(crate) struct Seam<C: Calls> {
    calls: PhantomData<C>,
}

impl<C: Calls> Seam<C> {
    /// A seam over the running core's displays.
    ///
    /// # Safety
    /// The core is up, and the seam is handed only to one [`crate::display`]
    /// call, on displays that are live (from the caller of a `*_with`
    /// function, or allocated by that call) or NULL.
    #[must_use]
    const unsafe fn new() -> Self {
        Self { calls: PhantomData }
    }
}

impl<C: Calls> Host for Seam<C> {
    type Display = Display;
    type Data = NonNull<c_void>;
    type Swap = NonNull<c_void>;
    type Link = NonNull<*mut c_void>;
    type Callback = draw_callback;
    type Graphics = Opaque;
    type Effect = Opaque;
    type Param = Opaque;

    fn alloc(&mut self) -> Display {
        // SAFETY: bzalloc(): bmalloc() never returns NULL (it crashes on
        // failure), and the block is zeroed at its full size.
        unsafe {
            let size = obs_display_seam_size();
            let mem = bmalloc(size);
            ptr::write_bytes(mem.cast::<u8>(), 0, size);
            Display(NonNull::new_unchecked(mem))
        }
    }
    fn dealloc(&mut self, display: Display) {
        // SAFETY: the block alloc() gave.
        unsafe { bfree(display.ptr()) }
    }
    fn log_error(&mut self, message: &'static CStr) {
        C::log_error(message);
    }

    fn init_mutex_values(&mut self, d: Display) {
        // SAFETY: a live display.
        unsafe { obs_display_seam_init_mutex_values(d.ptr()) }
    }
    fn callbacks_mutex_init(&mut self, d: Display) -> c_int {
        // SAFETY: a live display.
        unsafe { obs_display_seam_callbacks_mutex_init(d.ptr()) }
    }
    fn info_mutex_init(&mut self, d: Display) -> c_int {
        // SAFETY: a live display.
        unsafe { obs_display_seam_info_mutex_init(d.ptr()) }
    }
    fn callbacks_mutex_destroy(&mut self, d: Display) {
        // SAFETY: a live display.
        unsafe { obs_display_seam_callbacks_mutex_destroy(d.ptr()) }
    }
    fn info_mutex_destroy(&mut self, d: Display) {
        // SAFETY: a live display.
        unsafe { obs_display_seam_info_mutex_destroy(d.ptr()) }
    }
    fn lock_callbacks(&mut self, d: Display) {
        // SAFETY: a live display with its mutex made.
        let _ = unsafe { obs_display_seam_lock_callbacks(d.ptr()) };
    }
    fn unlock_callbacks(&mut self, d: Display) {
        // SAFETY: held by this thread.
        let _ = unsafe { obs_display_seam_unlock_callbacks(d.ptr()) };
    }
    fn lock_info(&mut self, d: Display) {
        // SAFETY: a live display with its mutex made.
        unsafe { obs_display_size_lock(d.ptr()) }
    }
    fn unlock_info(&mut self, d: Display) {
        // SAFETY: held by this thread.
        unsafe { obs_display_size_unlock(d.ptr()) }
    }

    fn host_clear_workaround(&mut self) -> bool {
        C::host_clear_workaround()
    }
    fn host_clear_color_always_srgb(&mut self) -> bool {
        C::host_clear_color_always_srgb()
    }

    fn use_clear_workaround(&mut self, d: Display) -> bool {
        // SAFETY: a live display.
        unsafe { obs_display_seam_get_use_clear_workaround(d.ptr()) }
    }
    fn set_use_clear_workaround(&mut self, d: Display, value: bool) {
        // SAFETY: a live display.
        unsafe { obs_display_seam_set_use_clear_workaround(d.ptr(), value) }
    }
    fn swap(&mut self, d: Display) -> Option<NonNull<c_void>> {
        // SAFETY: a live display.
        NonNull::new(unsafe { obs_display_seam_get_swap(d.ptr()) })
    }
    fn set_swap(&mut self, d: Display, value: Option<NonNull<c_void>>) {
        let value = value.map_or(ptr::null_mut(), NonNull::as_ptr);
        // SAFETY: a live display.
        unsafe { obs_display_seam_set_swap(d.ptr(), value) }
    }
    fn cx(&mut self, d: Display) -> u32 {
        // SAFETY: a live display; the field pointer is read at once.
        unsafe { obs_display_size_fields(d.ptr()).cx.read() }
    }
    fn cy(&mut self, d: Display) -> u32 {
        // SAFETY: as cx.
        unsafe { obs_display_size_fields(d.ptr()).cy.read() }
    }
    fn set_cx(&mut self, d: Display, value: u32) {
        // SAFETY: as cx.
        unsafe { obs_display_size_fields(d.ptr()).cx.write(value) }
    }
    fn set_cy(&mut self, d: Display, value: u32) {
        // SAFETY: as cx.
        unsafe { obs_display_size_fields(d.ptr()).cy.write(value) }
    }
    fn next_cx(&mut self, d: Display) -> u32 {
        // SAFETY: a live display.
        unsafe { obs_display_seam_get_next_cx(d.ptr()) }
    }
    fn next_cy(&mut self, d: Display) -> u32 {
        // SAFETY: a live display.
        unsafe { obs_display_seam_get_next_cy(d.ptr()) }
    }
    fn set_next_cx(&mut self, d: Display, value: u32) {
        // SAFETY: a live display.
        unsafe { obs_display_seam_set_next_cx(d.ptr(), value) }
    }
    fn set_next_cy(&mut self, d: Display, value: u32) {
        // SAFETY: a live display.
        unsafe { obs_display_seam_set_next_cy(d.ptr(), value) }
    }
    fn update_color_space(&mut self, d: Display) -> bool {
        // SAFETY: a live display.
        unsafe { obs_display_seam_get_update_color_space(d.ptr()) }
    }
    fn set_update_color_space(&mut self, d: Display, value: bool) {
        // SAFETY: a live display.
        unsafe { obs_display_seam_set_update_color_space(d.ptr(), value) }
    }
    fn enabled(&mut self, d: Display) -> bool {
        // SAFETY: a live display.
        unsafe { obs_display_seam_get_enabled(d.ptr()) }
    }
    fn set_enabled(&mut self, d: Display, value: bool) {
        // SAFETY: a live display.
        unsafe { obs_display_seam_set_enabled(d.ptr(), value) }
    }
    fn background_color(&mut self, d: Display) -> u32 {
        // SAFETY: a live display.
        unsafe { obs_display_seam_get_background_color(d.ptr()) }
    }
    fn set_background_color(&mut self, d: Display, value: u32) {
        // SAFETY: a live display.
        unsafe { obs_display_seam_set_background_color(d.ptr(), value) }
    }

    fn callbacks_push(&mut self, d: Display, callback: draw_callback) {
        // SAFETY: a live display; the callback lives for the call.
        unsafe { obs_display_seam_callbacks_push(d.ptr(), &raw const callback) }
    }
    fn callbacks_erase_item(&mut self, d: Display, callback: draw_callback) {
        // SAFETY: as push.
        unsafe { obs_display_seam_callbacks_erase_item(d.ptr(), &raw const callback) }
    }
    fn callbacks_num(&mut self, d: Display) -> usize {
        // SAFETY: a live display.
        unsafe { obs_display_seam_callbacks_num(d.ptr()) }
    }
    fn callbacks_get(&mut self, d: Display, idx: usize) -> draw_callback {
        // SAFETY: idx is below the count, so the item is in the array; it
        // is copied out at once, and no reference to it is kept.
        unsafe { obs_display_seam_callbacks_get(d.ptr(), idx).read() }
    }
    fn callbacks_free(&mut self, d: Display) {
        // SAFETY: a live display.
        unsafe { obs_display_seam_callbacks_free(d.ptr()) }
    }
    fn call(&mut self, callback: draw_callback, cx: u32, cy: u32) {
        if let Some(draw) = callback.draw {
            // SAFETY: the plugin's callback with its own param, as added.
            unsafe { draw(callback.param, cx, cy) }
        }
    }

    fn lock_displays(&mut self) {
        // SAFETY: the core's mutex.
        let _ = unsafe { obs_display_seam_lock_displays() };
    }
    fn unlock_displays(&mut self) {
        // SAFETY: held by this thread.
        let _ = unsafe { obs_display_seam_unlock_displays() };
    }
    fn first_display(&mut self) -> Option<Display> {
        // SAFETY: the core's list, under displays_mutex.
        NonNull::new(unsafe { obs_display_seam_get_first_display() }).map(Display)
    }
    fn set_first_display(&mut self, value: Option<Display>) {
        // SAFETY: as first_display.
        unsafe { obs_display_seam_set_first_display(display_or_null(value)) }
    }
    fn first_display_ref(&mut self) -> NonNull<*mut c_void> {
        // SAFETY: the address of a field of the core, never NULL.
        unsafe { NonNull::new_unchecked(obs_display_seam_first_display_ref()) }
    }
    fn next(&mut self, d: Display) -> Option<Display> {
        // SAFETY: a live display, under displays_mutex.
        NonNull::new(unsafe { obs_display_seam_get_next(d.ptr()) }).map(Display)
    }
    fn set_next(&mut self, d: Display, value: Option<Display>) {
        // SAFETY: as next.
        unsafe { obs_display_seam_set_next(d.ptr(), display_or_null(value)) }
    }
    fn next_ref(&mut self, d: Display) -> NonNull<*mut c_void> {
        // SAFETY: the address of a field of a live display, never NULL.
        unsafe { NonNull::new_unchecked(obs_display_seam_next_ref(d.ptr())) }
    }
    fn prev_next(&mut self, d: Display) -> Option<NonNull<*mut c_void>> {
        // SAFETY: as next.
        NonNull::new(unsafe { obs_display_seam_get_prev_next(d.ptr()) })
    }
    fn set_prev_next(&mut self, d: Display, value: Option<NonNull<*mut c_void>>) {
        let value = value.map_or(ptr::null_mut(), NonNull::as_ptr);
        // SAFETY: as next.
        unsafe { obs_display_seam_set_prev_next(d.ptr(), value) }
    }
    fn store_link(&mut self, link: NonNull<*mut c_void>, value: Option<Display>) {
        // SAFETY: a link of the list (the core's head or a display's next),
        // under displays_mutex.
        unsafe { link.as_ptr().write(display_or_null(value)) }
    }

    fn graphics(&mut self) -> Opaque {
        // SAFETY: the core's field.
        Opaque(unsafe { obs_display_seam_get_graphics() })
    }
    fn solid_effect(&mut self) -> Opaque {
        // SAFETY: the core's field.
        Opaque(unsafe { obs_display_seam_get_solid_effect() })
    }
    fn data_cx(&mut self, data: NonNull<c_void>) -> u32 {
        // SAFETY: the caller's live graphics data.
        unsafe { obs_display_seam_get_init_cx(data.as_ptr()) }
    }
    fn data_cy(&mut self, data: NonNull<c_void>) -> u32 {
        // SAFETY: as data_cx.
        unsafe { obs_display_seam_get_init_cy(data.as_ptr()) }
    }

    fn enter_graphics(&mut self) {
        C::enter_graphics();
    }
    fn leave_graphics(&mut self) {
        C::leave_graphics();
    }
    fn enter_context(&mut self, graphics: Opaque) {
        // SAFETY: the core's graphics, as the C passes it.
        unsafe { C::enter_context(graphics.0) }
    }
    fn leave_context(&mut self) {
        C::leave_context();
    }
    fn swapchain_create(&mut self, data: NonNull<c_void>) -> Option<NonNull<c_void>> {
        // SAFETY: the caller's live graphics data.
        NonNull::new(unsafe { C::swapchain_create(data.as_ptr()) })
    }
    fn swapchain_destroy(&mut self, swap: NonNull<c_void>) {
        // SAFETY: the display's own swap chain.
        unsafe { C::swapchain_destroy(swap.as_ptr()) }
    }
    fn load_swapchain(&mut self, swap: Option<NonNull<c_void>>) {
        // SAFETY: the display's own swap chain or NULL.
        unsafe { C::load_swapchain(swap.map_or(ptr::null_mut(), NonNull::as_ptr)) }
    }
    fn resize(&mut self, cx: u32, cy: u32) {
        C::resize(cx, cy);
    }
    fn gs_update_color_space(&mut self) {
        C::update_color_space();
    }
    fn is_present_ready(&mut self) -> bool {
        C::is_present_ready()
    }
    fn begin_scene(&mut self) {
        C::begin_scene();
    }
    fn color_space(&mut self) -> c_int {
        C::color_space()
    }
    fn clear(&mut self, flags: u32, color: core::Vec4, depth: f32, stencil: u8) {
        C::clear(flags, &vec4::from(color), depth, stencil);
    }
    fn enable_depth_test(&mut self, enable: bool) {
        C::enable_depth_test(enable);
    }
    fn set_cull_mode(&mut self, mode: c_int) {
        C::set_cull_mode(mode);
    }
    fn ortho(&mut self, left: f32, right: f32, top: f32, bottom: f32, znear: f32, zfar: f32) {
        C::ortho(left, right, top, bottom, znear, zfar);
    }
    fn set_viewport(&mut self, x: c_int, y: c_int, width: c_int, height: c_int) {
        C::set_viewport(x, y, width, height);
    }
    fn param_by_name(&mut self, effect: Opaque, name: &'static CStr) -> Opaque {
        // SAFETY: the core's solid effect, as the C passes it.
        Opaque(unsafe { C::param_by_name(effect.0, name.as_ptr()) })
    }
    fn effect_set_vec4(&mut self, param: Opaque, value: core::Vec4) {
        // SAFETY: the param the lookup gave.
        unsafe { C::effect_set_vec4(param.0, &vec4::from(value)) }
    }
    fn effect_loop(&mut self, effect: Opaque, technique: &'static CStr) -> bool {
        // SAFETY: as param_by_name.
        unsafe { C::effect_loop(effect.0, technique.as_ptr()) }
    }
    fn draw_sprite(&mut self, cx: u32, cy: u32) {
        C::draw_sprite(cx, cy);
    }
    fn end_scene(&mut self) {
        C::end_scene();
    }
    fn present(&mut self) {
        C::present();
    }
}

/// `obs_display_init()` on `C`.
///
/// # Safety
/// `display` is a live, zeroed or freed display; `graphics_data` is NULL or
/// a live `struct gs_init_data`.
pub unsafe fn init_with<C: Calls>(display: *mut c_void, graphics_data: *const c_void) -> bool {
    // SAFETY: this function's contract, which its caller keeps.
    let mut seam = unsafe { Seam::<C>::new() };
    let Some(display) = NonNull::new(display) else {
        // The C dereferences it; a NULL display is never passed.
        return false;
    };
    core::init(
        &mut seam,
        Display(display),
        NonNull::new(graphics_data.cast_mut()),
    )
}

/// `obs_display_create()` on `C`.
///
/// # Safety
/// The core is up; `graphics_data` is NULL or a live `struct gs_init_data`.
pub unsafe fn create_with<C: Calls>(
    graphics_data: *const c_void,
    background_color: u32,
) -> *mut c_void {
    // SAFETY: this function's contract, which its caller keeps.
    let mut seam = unsafe { Seam::<C>::new() };
    display_or_null(core::create(
        &mut seam,
        NonNull::new(graphics_data.cast_mut()),
        background_color,
    ))
}

/// `obs_display_free()` on `C`.
///
/// # Safety
/// `display` is a live display made by [`init_with`].
pub unsafe fn free_with<C: Calls>(display: *mut c_void) {
    // SAFETY: this function's contract, which its caller keeps.
    let mut seam = unsafe { Seam::<C>::new() };
    if let Some(display) = NonNull::new(display) {
        core::free(&mut seam, Display(display));
    }
}

/// `obs_display_destroy()` on `C`.
///
/// # Safety
/// `display` is NULL or a live display from [`create_with`].
pub unsafe fn destroy_with<C: Calls>(display: *mut c_void) {
    // SAFETY: this function's contract, which its caller keeps.
    let mut seam = unsafe { Seam::<C>::new() };
    core::destroy(&mut seam, NonNull::new(display).map(Display));
}

/// `obs_display_resize()` on `C`.
///
/// # Safety
/// `display` is NULL or a live display.
pub unsafe fn resize_with<C: Calls>(display: *mut c_void, cx: u32, cy: u32) {
    // SAFETY: this function's contract, which its caller keeps.
    let mut seam = unsafe { Seam::<C>::new() };
    core::resize(&mut seam, NonNull::new(display).map(Display), cx, cy);
}

/// `obs_display_update_color_space()` on `C`.
///
/// # Safety
/// As [`resize_with`].
pub unsafe fn update_color_space_with<C: Calls>(display: *mut c_void) {
    // SAFETY: this function's contract, which its caller keeps.
    let mut seam = unsafe { Seam::<C>::new() };
    core::update_color_space(&mut seam, NonNull::new(display).map(Display));
}

/// `obs_display_add_draw_callback()` on `C`.
///
/// # Safety
/// As [`resize_with`].
pub unsafe fn add_draw_callback_with<C: Calls>(
    display: *mut c_void,
    draw: Option<DrawFn>,
    param: *mut c_void,
) {
    // SAFETY: this function's contract, which its caller keeps.
    let mut seam = unsafe { Seam::<C>::new() };
    core::add_draw_callback(
        &mut seam,
        NonNull::new(display).map(Display),
        draw_callback { draw, param },
    );
}

/// `obs_display_remove_draw_callback()` on `C`.
///
/// # Safety
/// As [`resize_with`].
pub unsafe fn remove_draw_callback_with<C: Calls>(
    display: *mut c_void,
    draw: Option<DrawFn>,
    param: *mut c_void,
) {
    // SAFETY: this function's contract, which its caller keeps.
    let mut seam = unsafe { Seam::<C>::new() };
    core::remove_draw_callback(
        &mut seam,
        NonNull::new(display).map(Display),
        draw_callback { draw, param },
    );
}

/// `render_display()` on `C`.
///
/// # Safety
/// As [`resize_with`], on the thread that holds the graphics context.
pub unsafe fn render_with<C: Calls>(display: *mut c_void) {
    // SAFETY: this function's contract, which its caller keeps.
    let mut seam = unsafe { Seam::<C>::new() };
    core::render(&mut seam, NonNull::new(display).map(Display));
}

/// `bool obs_display_init(struct obs_display *display, const struct
/// gs_init_data *graphics_data)`.
///
/// # Safety
/// See [`init_with`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_display_init(
    display: *mut c_void,
    graphics_data: *const c_void,
) -> bool {
    // SAFETY: the caller's contract.
    unsafe { init_with::<Libobs>(display, graphics_data) }
}

/// `obs_display_t *obs_display_create(const struct gs_init_data
/// *graphics_data, uint32_t background_color)`.
///
/// # Safety
/// See [`create_with`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_display_create(
    graphics_data: *const c_void,
    background_color: u32,
) -> *mut c_void {
    // SAFETY: the caller's contract.
    unsafe { create_with::<Libobs>(graphics_data, background_color) }
}

/// `void obs_display_free(obs_display_t *display)`.
///
/// # Safety
/// See [`free_with`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_display_free(display: *mut c_void) {
    // SAFETY: the caller's contract.
    unsafe { free_with::<Libobs>(display) }
}

/// `void obs_display_destroy(obs_display_t *display)`.
///
/// # Safety
/// See [`destroy_with`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_display_destroy(display: *mut c_void) {
    // SAFETY: the caller's contract.
    unsafe { destroy_with::<Libobs>(display) }
}

/// `void obs_display_resize(obs_display_t *display, uint32_t cx, uint32_t
/// cy)`.
///
/// # Safety
/// See [`resize_with`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_display_resize(display: *mut c_void, cx: u32, cy: u32) {
    // SAFETY: the caller's contract.
    unsafe { resize_with::<Libobs>(display, cx, cy) }
}

/// `void obs_display_update_color_space(obs_display_t *display)`.
///
/// # Safety
/// See [`update_color_space_with`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_display_update_color_space(display: *mut c_void) {
    // SAFETY: the caller's contract.
    unsafe { update_color_space_with::<Libobs>(display) }
}

/// `void obs_display_add_draw_callback(obs_display_t *display, void
/// (*draw)(void *param, uint32_t cx, uint32_t cy), void *param)`.
///
/// # Safety
/// See [`add_draw_callback_with`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_display_add_draw_callback(
    display: *mut c_void,
    draw: Option<DrawFn>,
    param: *mut c_void,
) {
    // SAFETY: the caller's contract.
    unsafe { add_draw_callback_with::<Libobs>(display, draw, param) }
}

/// `void obs_display_remove_draw_callback(obs_display_t *display, void
/// (*draw)(void *param, uint32_t cx, uint32_t cy), void *param)`.
///
/// # Safety
/// See [`remove_draw_callback_with`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_display_remove_draw_callback(
    display: *mut c_void,
    draw: Option<DrawFn>,
    param: *mut c_void,
) {
    // SAFETY: the caller's contract.
    unsafe { remove_draw_callback_with::<Libobs>(display, draw, param) }
}

/// `void render_display(struct obs_display *display)`.
///
/// # Safety
/// See [`render_with`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn render_display(display: *mut c_void) {
    // SAFETY: the caller's contract.
    unsafe { render_with::<Libobs>(display) }
}
