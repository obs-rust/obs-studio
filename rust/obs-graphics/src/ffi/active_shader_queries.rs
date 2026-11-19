//! Thin ABI shims over the real device-query TLS, logging and dispatch seam.
use crate::active_shader_queries::{self as core, Entry, Host};
use std::ffi::{c_char, c_void};

unsafe extern "C" {
    fn obs_graphics_device_queries_context() -> *mut c_void;
    fn obs_graphics_device_queries_debug(function: *const c_char);
    fn obs_graphics_device_queries_vertex_shader(context: *mut c_void) -> *mut c_void;
    fn obs_graphics_device_queries_pixel_shader(context: *mut c_void) -> *mut c_void;
}

struct Context(*mut c_void);

fn current() -> Context {
    // SAFETY: reads the calling thread's context without dereferencing it.
    Context(unsafe { obs_graphics_device_queries_context() })
}

impl Host for Context {
    type Shader = *mut c_void;
    fn has_context(&self) -> bool {
        !self.0.is_null()
    }
    fn debug(&mut self, entry: Entry) {
        let name = match entry {
            Entry::Vertex => c"gs_get_vertex_shader",
            Entry::Pixel => c"gs_get_pixel_shader",
        };
        // SAFETY: the function name is static and NUL-terminated.
        unsafe { obs_graphics_device_queries_debug(name.as_ptr()) }
    }
    fn dispatch(&mut self, entry: Entry) -> Self::Shader {
        // SAFETY: the core checked NULL before any projection. A live graphics
        // context supplies the mandatory callback with its real const-device ABI.
        unsafe {
            match entry {
                Entry::Vertex => obs_graphics_device_queries_vertex_shader(self.0),
                Entry::Pixel => obs_graphics_device_queries_pixel_shader(self.0),
            }
        }
    }
}

/// Borrow the current vertex shader, or return NULL without a context.
/// # Safety
/// Any current context, device and mandatory backend callback must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_get_vertex_shader() -> *mut c_void {
    core::shader(&mut current(), Entry::Vertex)
}

/// Borrow the current pixel shader, or return NULL without a context.
/// # Safety
/// Any current context, device and mandatory backend callback must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_get_pixel_shader() -> *mut c_void {
    core::shader(&mut current(), Entry::Pixel)
}
