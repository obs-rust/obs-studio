//! Thin public ABI over the safe device-query core and private native adapters.
use crate::device_queries::{self as core, Entry, Host};
use std::ffi::{c_char, c_void};

unsafe extern "C" {
    fn obs_graphics_device_queries_context() -> *mut c_void;
    fn obs_graphics_device_queries_debug(function: *const c_char);
    fn obs_graphics_device_queries_type(context: *mut c_void) -> i32;
    fn obs_graphics_device_queries_has_adapter_count(context: *mut c_void) -> bool;
    fn obs_graphics_device_queries_count(context: *mut c_void) -> u32;
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
        // SAFETY: the function name is static and NUL-terminated.
        unsafe { obs_graphics_device_queries_debug(entry.c_name().as_ptr()) }
    }
    fn device_type(&mut self) -> i32 {
        // SAFETY: the safe core checked context before field access; a live
        // graphics backend supplies the required device_get_type callback.
        unsafe { obs_graphics_device_queries_type(self.0) }
    }
    fn has_adapter_count(&self) -> bool {
        // SAFETY: the safe core checked context before reading this field.
        unsafe { obs_graphics_device_queries_has_adapter_count(self.0) }
    }
    fn adapter_count(&mut self) -> u32 {
        // SAFETY: the safe core checked context and callback presence first.
        unsafe { obs_graphics_device_queries_count(self.0) }
    }
}

/// Return the backend device type, or -1 without a current context.
/// # Safety
/// A current context must be live and supply its required backend callbacks.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_get_device_type() -> i32 {
    core::get_device_type(&mut current()).unwrap_or(-1)
}

/// Return the adapter count, or zero without a context or optional callback.
/// # Safety
/// A current context and any supplied backend callback must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_get_adapter_count() -> u32 {
    core::get_adapter_count(&mut current()).unwrap_or(0)
}
