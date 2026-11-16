//! The C ABI of the cull and colour calls.
//!
//! The context read, the debug line and the device calls are one-line C
//! adapters (`libobs/graphics/raster-state-entrypoints-fields.c.h`), which
//! keep the private graphics layout and the device ABI in C. Each call reads
//! this thread's context once, when it is made, and makes at most one device
//! call; nothing is cached or allocated.
use crate::raster_entrypoints::{self as raster, Entry, Host};
use std::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_graphics_raster_context() -> *mut c_void;
    fn obs_graphics_raster_debug(function: *const c_char);
    fn obs_graphics_raster_set_cull_mode(graphics: *mut c_void, mode: c_int);
    fn obs_graphics_raster_get_cull_mode(graphics: *mut c_void) -> c_int;
    fn obs_graphics_raster_enable_color(
        graphics: *mut c_void,
        red: bool,
        green: bool,
        blue: bool,
        alpha: bool,
    );
}

/// This thread's graphics context, as read at the start of the call.
struct Context(*mut c_void);

fn current() -> Context {
    // SAFETY: reads the calling thread's context pointer; nothing is retained
    // past the call.
    Context(unsafe { obs_graphics_raster_context() })
}

// SAFETY (every device call below): the core calls them only after
// `has_context` saw a non-NULL context, which stays current on this thread for
// the call; the device and its exports belong to the backend.
impl Host for Context {
    fn has_context(&self) -> bool {
        !self.0.is_null()
    }
    fn debug(&mut self, entry: Entry) {
        // SAFETY: a static terminated name, read during the call.
        unsafe { obs_graphics_raster_debug(entry.c_name().as_ptr()) }
    }
    fn set_cull_mode(&mut self, mode: i32) {
        unsafe { obs_graphics_raster_set_cull_mode(self.0, mode) }
    }
    fn get_cull_mode(&mut self) -> i32 {
        unsafe { obs_graphics_raster_get_cull_mode(self.0) }
    }
    fn enable_color(&mut self, red: bool, green: bool, blue: bool, alpha: bool) {
        unsafe { obs_graphics_raster_enable_color(self.0, red, green, blue, alpha) }
    }
}

/// Set the current device's cull mode; the mode is passed on unchecked.
/// # Safety
/// A current context must hold a live device and its exports.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_set_cull_mode(mode: c_int) {
    raster::set_cull_mode(&mut current(), mode);
}

/// The current device's cull mode; `GS_NEITHER` without a context.
/// # Safety
/// A current context must hold a live device and its exports.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_get_cull_mode() -> c_int {
    raster::get_cull_mode(&mut current())
}

/// Set which colour channels the current device writes.
/// # Safety
/// A current context must hold a live device and its exports.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_enable_color(red: bool, green: bool, blue: bool, alpha: bool) {
    raster::enable_color(&mut current(), red, green, blue, alpha);
}
