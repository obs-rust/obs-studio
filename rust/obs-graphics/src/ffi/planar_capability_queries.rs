//! Thin ABI shims using the existing device-query TLS/logging/field seam.
use crate::planar_capability_queries::{self as core, Entry, Host};
use std::ffi::{c_char, c_void};

unsafe extern "C" {
    fn obs_graphics_device_queries_context() -> *mut c_void;
    fn obs_graphics_device_queries_debug(function: *const c_char);
    fn obs_graphics_device_queries_has_nv12(context: *mut c_void) -> bool;
    fn obs_graphics_device_queries_nv12(context: *mut c_void) -> bool;
    fn obs_graphics_device_queries_has_p010(context: *mut c_void) -> bool;
    fn obs_graphics_device_queries_p010(context: *mut c_void) -> bool;
}

struct Context(*mut c_void);

fn current() -> Context {
    // SAFETY: reads the calling thread's context without dereferencing it.
    Context(unsafe { obs_graphics_device_queries_context() })
}

impl Host for Context {
    fn has_context(&self) -> bool {
        !self.0.is_null()
    }
    fn debug(&mut self, entry: Entry) {
        let name = match entry {
            Entry::Nv12 => c"gs_nv12_available",
            Entry::P010 => c"gs_p010_available",
        };
        // SAFETY: the function name is static and NUL-terminated.
        unsafe { obs_graphics_device_queries_debug(name.as_ptr()) }
    }
    fn has_callback(&self, entry: Entry) -> bool {
        // SAFETY: the core checked the context before any field projection.
        unsafe {
            match entry {
                Entry::Nv12 => obs_graphics_device_queries_has_nv12(self.0),
                Entry::P010 => obs_graphics_device_queries_has_p010(self.0),
            }
        }
    }
    fn dispatch(&mut self, entry: Entry) -> bool {
        // SAFETY: the core checked context and the matching callback first.
        unsafe {
            match entry {
                Entry::Nv12 => obs_graphics_device_queries_nv12(self.0),
                Entry::P010 => obs_graphics_device_queries_p010(self.0),
            }
        }
    }
}

/// Query the optional NV12 backend capability.
/// # Safety
/// A current context and any supplied backend callback must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_nv12_available() -> bool {
    core::available(&mut current(), Entry::Nv12)
}

/// Query the optional P010 backend capability.
/// # Safety
/// A current context and any supplied backend callback must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_p010_available() -> bool {
    core::available(&mut current(), Entry::P010)
}
