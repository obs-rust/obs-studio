//! Thin target-size ABI over safe dispatch and private native layout adapters.
use crate::target_size::{self as core, Entry, Host};
use std::ffi::{c_char, c_void};

unsafe extern "C" {
    fn obs_graphics_target_size_context() -> *mut c_void;
    fn obs_graphics_target_size_debug(function: *const c_char);
    fn obs_graphics_target_size_size(context: *mut c_void, x: *mut u32, y: *mut u32);
    fn obs_graphics_target_size_width(context: *mut c_void) -> u32;
    fn obs_graphics_target_size_height(context: *mut c_void) -> u32;
}
struct Context(*mut c_void);
fn current() -> Context {
    // SAFETY: reads the calling thread's context without dereferencing it.
    Context(unsafe { obs_graphics_target_size_context() })
}
impl Host for Context {
    type Output = *mut u32;
    fn has_context(&self) -> bool {
        !self.0.is_null()
    }
    fn debug(&mut self, entry: Entry) {
        // SAFETY: the function name is static and NUL-terminated.
        unsafe { obs_graphics_target_size_debug(entry.c_name().as_ptr()) }
    }
    fn size(&mut self, x: *mut u32, y: *mut u32) {
        // SAFETY: the core checked context before field access. The caller
        // supplies backend-compatible outputs, forwarded without dereferencing.
        unsafe { obs_graphics_target_size_size(self.0, x, y) }
    }
    fn width(&mut self) -> u32 {
        // SAFETY: the core checked context; the backend callback is required.
        unsafe { obs_graphics_target_size_width(self.0) }
    }
    fn height(&mut self) -> u32 {
        // SAFETY: the core checked context; the backend callback is required.
        unsafe { obs_graphics_target_size_height(self.0) }
    }
}
/// Query both dimensions through the current backend.
/// # Safety
/// A current context must be live and supply its required callback. Outputs,
/// including NULL or aliased pointers, must satisfy that backend's contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_get_size(x: *mut u32, y: *mut u32) {
    core::get_size(&mut current(), x, y);
}
/// Query width, returning zero without a current context.
/// # Safety
/// A current context must be live and supply its required backend callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_get_width() -> u32 {
    core::get_width(&mut current())
}
/// Query height, returning zero without a current context.
/// # Safety
/// A current context must be live and supply its required backend callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_get_height() -> u32 {
    core::get_height(&mut current())
}
