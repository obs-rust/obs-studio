//! The C ABI of the frame and present calls.
//!
//! The context read, the debug line and the device calls are one-line C
//! adapters (`libobs/graphics/present-entrypoints-fields.c.h`), which keep
//! the private graphics layout and the device ABI in C. Each call reads this
//! thread's context once, when it is made, and makes at most one device
//! call; nothing is cached or allocated.
use crate::present_entrypoints::{self as present, Entry, Host};
use std::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_graphics_present_context() -> *mut c_void;
    fn obs_graphics_present_debug(function: *const c_char);
    fn obs_graphics_present_begin_frame(graphics: *mut c_void);
    fn obs_graphics_present_begin_scene(graphics: *mut c_void);
    fn obs_graphics_present_draw(graphics: *mut c_void, mode: c_int, start: u32, count: u32);
    fn obs_graphics_present_end_scene(graphics: *mut c_void);
    fn obs_graphics_present_load_swapchain(graphics: *mut c_void, swapchain: *mut c_void);
    fn obs_graphics_present_clear(
        graphics: *mut c_void,
        flags: u32,
        color: *const c_void,
        depth: f32,
        stencil: u8,
    );
    fn obs_graphics_present_is_present_ready(graphics: *mut c_void) -> bool;
    fn obs_graphics_present_present(graphics: *mut c_void);
    fn obs_graphics_present_flush(graphics: *mut c_void);
}

/// This thread's graphics context, as read at the start of the call.
struct Context(*mut c_void);

fn current() -> Context {
    // SAFETY: reads the calling thread's context pointer; nothing is retained
    // past the call.
    Context(unsafe { obs_graphics_present_context() })
}

// SAFETY (every device call below): the core calls them only after
// `has_context` saw a non-NULL context, which stays current on this thread for
// the call; the device and its exports belong to the backend.
impl Host for Context {
    type Swapchain = *mut c_void;
    type Color = *const c_void;

    fn has_context(&self) -> bool {
        !self.0.is_null()
    }
    fn debug(&mut self, entry: Entry) {
        // SAFETY: a static terminated name, read during the call.
        unsafe { obs_graphics_present_debug(entry.c_name().as_ptr()) }
    }
    fn begin_frame(&mut self) {
        unsafe { obs_graphics_present_begin_frame(self.0) }
    }
    fn begin_scene(&mut self) {
        unsafe { obs_graphics_present_begin_scene(self.0) }
    }
    fn draw(&mut self, mode: i32, start_vert: u32, num_verts: u32) {
        unsafe { obs_graphics_present_draw(self.0, mode, start_vert, num_verts) }
    }
    fn end_scene(&mut self) {
        unsafe { obs_graphics_present_end_scene(self.0) }
    }
    fn load_swapchain(&mut self, swapchain: *mut c_void) {
        unsafe { obs_graphics_present_load_swapchain(self.0, swapchain) }
    }
    fn clear(&mut self, flags: u32, color: *const c_void, depth: f32, stencil: u8) {
        unsafe { obs_graphics_present_clear(self.0, flags, color, depth, stencil) }
    }
    fn is_present_ready(&mut self) -> bool {
        unsafe { obs_graphics_present_is_present_ready(self.0) }
    }
    fn present(&mut self) {
        unsafe { obs_graphics_present_present(self.0) }
    }
    fn flush(&mut self) {
        unsafe { obs_graphics_present_flush(self.0) }
    }
}

/// Start a frame on the current device.
/// # Safety
/// A current context must hold a live device and its exports.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_begin_frame() {
    present::begin_frame(&mut current());
}

/// Start a scene on the current device.
/// # Safety
/// A current context must hold a live device and its exports.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_begin_scene() {
    present::begin_scene(&mut current());
}

/// Draw with the current device; the mode is passed on unchecked.
/// # Safety
/// A current context must hold a live device and its exports.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_draw(draw_mode: c_int, start_vert: u32, num_verts: u32) {
    present::draw(&mut current(), draw_mode, start_vert, num_verts);
}

/// End a scene on the current device.
/// # Safety
/// A current context must hold a live device and its exports.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_end_scene() {
    present::end_scene(&mut current());
}

/// Load a swapchain (NULL too) on the current device.
/// # Safety
/// A current context must hold a live device and its exports; the swapchain
/// is the backend's to check.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_load_swapchain(swapchain: *mut c_void) {
    present::load_swapchain(&mut current(), swapchain);
}

/// Clear with the current device; the colour pointer (NULL too) and the
/// depth's bits are passed on unchanged.
/// # Safety
/// A current context must hold a live device and its exports; the colour is
/// the backend's to read.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_clear(clear_flags: u32, color: *const c_void, depth: f32, stencil: u8) {
    present::clear(&mut current(), clear_flags, color, depth, stencil);
}

/// Whether the current device is ready to present; false without a context.
/// # Safety
/// A current context must hold a live device and its exports.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_is_present_ready() -> bool {
    present::is_present_ready(&mut current())
}

/// Present on the current device.
/// # Safety
/// A current context must hold a live device and its exports.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_present() {
    present::present(&mut current());
}

/// Flush the current device.
/// # Safety
/// A current context must hold a live device and its exports.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_flush() {
    present::flush(&mut current());
}
