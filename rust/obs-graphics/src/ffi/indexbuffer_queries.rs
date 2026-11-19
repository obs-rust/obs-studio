//! Thin ABI shims over the shared indexbuffer context, log and dispatch seam.
use crate::indexbuffer_queries::{self as core, Entry, Host, Rejection};
use std::ffi::{c_char, c_uint, c_void};

unsafe extern "C" {
    fn obs_graphics_indexbuffer_maintenance_context() -> *mut c_void;
    fn obs_graphics_indexbuffer_maintenance_debug(function: *const c_char, reason: *const c_char);
    fn obs_graphics_indexbuffer_maintenance_data(
        context: *mut c_void,
        buffer: *const c_void,
    ) -> *mut c_void;
    fn obs_graphics_indexbuffer_maintenance_type(
        context: *mut c_void,
        buffer: *const c_void,
    ) -> c_uint;
}

struct Context(*mut c_void);

fn current() -> Context {
    // SAFETY: reads the current thread's context without projecting any fields.
    Context(unsafe { obs_graphics_indexbuffer_maintenance_context() })
}

impl Host for Context {
    type Buffer = *const c_void;
    type Data = *mut c_void;
    fn has_context(&self) -> bool {
        !self.0.is_null()
    }
    fn debug(&mut self, entry: Entry, reason: Rejection) {
        let name = match entry {
            Entry::Data => c"gs_indexbuffer_get_data",
            Entry::Type => c"gs_indexbuffer_get_type",
        };
        let message = match reason {
            Rejection::Context => c"called while not in a graphics context",
            Rejection::Buffer => c"Null 'indexbuffer' parameter",
        };
        // SAFETY: both strings are static and NUL-terminated.
        unsafe { obs_graphics_indexbuffer_maintenance_debug(name.as_ptr(), message.as_ptr()) }
    }
    fn data(&mut self, buffer: *const c_void) -> Option<*mut c_void> {
        // SAFETY: the safe core checked context and buffer before field projection.
        let data = unsafe { obs_graphics_indexbuffer_maintenance_data(self.0, buffer) };
        (!data.is_null()).then_some(data)
    }
    fn index_type(&mut self, buffer: *const c_void) -> u32 {
        // SAFETY: the safe core checked context and buffer before field projection.
        unsafe { obs_graphics_indexbuffer_maintenance_type(self.0, buffer) }
    }
}

fn optional(buffer: *const c_void) -> Option<*const c_void> {
    (!buffer.is_null()).then_some(buffer)
}

/// Forward the backend's data pointer, including a successful NULL result.
/// # Safety
/// A current context must have a live backend and mandatory callback; a non-NULL
/// buffer must meet that backend's contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_indexbuffer_get_data(buffer: *const c_void) -> *mut c_void {
    core::get_data(&mut current(), optional(buffer)).unwrap_or(std::ptr::null_mut())
}

/// Forward the backend's index enum, preserving the C enum-cast-zero rejection.
/// # Safety
/// A current context must have a live backend and mandatory callback; a non-NULL
/// buffer must meet that backend's contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_indexbuffer_get_type(buffer: *const c_void) -> c_uint {
    core::get_type(&mut current(), optional(buffer))
}
