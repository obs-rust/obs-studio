//! Thin ABI and private field access for indexbuffer maintenance.
use crate::indexbuffer_maintenance::{self as core, Entry, Host, Rejection};
use std::ffi::{c_char, c_void};

unsafe extern "C" {
    fn obs_graphics_indexbuffer_maintenance_context() -> *mut c_void;
    fn obs_graphics_indexbuffer_maintenance_debug(function: *const c_char, reason: *const c_char);
    fn obs_graphics_indexbuffer_maintenance_destroy(context: *mut c_void, buffer: *const c_void);
    fn obs_graphics_indexbuffer_maintenance_flush(context: *mut c_void, buffer: *const c_void);
    fn obs_graphics_indexbuffer_maintenance_direct(
        context: *mut c_void,
        buffer: *const c_void,
        data: *const c_void,
    );
    fn obs_graphics_indexbuffer_maintenance_count(
        context: *mut c_void,
        buffer: *const c_void,
    ) -> usize;
}

struct Context(*mut c_void);

fn current() -> Context {
    // SAFETY: reads the calling thread's context without dereferencing it.
    Context(unsafe { obs_graphics_indexbuffer_maintenance_context() })
}

fn optional(pointer: *const c_void) -> Option<*const c_void> {
    (!pointer.is_null()).then_some(pointer)
}

impl Host for Context {
    type Buffer = *const c_void;
    type Data = *const c_void;
    fn has_context(&self) -> bool {
        !self.0.is_null()
    }
    fn debug(&mut self, entry: Entry, reason: Rejection) {
        // SAFETY: both strings are static and NUL-terminated.
        unsafe {
            obs_graphics_indexbuffer_maintenance_debug(
                entry.c_name().as_ptr(),
                reason.message().as_ptr(),
            )
        }
    }
    fn destroy(&mut self, buffer: *const c_void) {
        // SAFETY: Rust validated context and buffer before any field adapter.
        unsafe { obs_graphics_indexbuffer_maintenance_destroy(self.0, buffer) }
    }
    fn flush(&mut self, buffer: *const c_void) {
        // SAFETY: Rust validated context and buffer before any field adapter.
        unsafe { obs_graphics_indexbuffer_maintenance_flush(self.0, buffer) }
    }
    fn direct(&mut self, buffer: *const c_void, data: *const c_void) {
        // SAFETY: Rust validated context, buffer and data before field access.
        unsafe { obs_graphics_indexbuffer_maintenance_direct(self.0, buffer, data) }
    }
    fn count(&mut self, buffer: *const c_void) -> usize {
        // SAFETY: Rust validated context and buffer before any field adapter.
        unsafe { obs_graphics_indexbuffer_maintenance_count(self.0, buffer) }
    }
}

/// Destroy a buffer, silently accepting NULL in a current context.
/// # Safety
/// A current context must have a live backend; a non-NULL buffer must meet its contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_indexbuffer_destroy(buffer: *mut c_void) {
    core::destroy(&mut current(), optional(buffer));
}

/// Flush a buffer through its backend.
/// # Safety
/// A current context must have a live backend; a non-NULL buffer must meet its contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_indexbuffer_flush(buffer: *mut c_void) {
    core::flush(&mut current(), optional(buffer));
}

/// Flush direct data through the backend.
/// # Safety
/// A current context must have a live backend; non-NULL buffer/data must meet its contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_indexbuffer_flush_direct(buffer: *mut c_void, data: *const c_void) {
    core::flush_direct(&mut current(), optional(buffer), optional(data));
}

/// Return the backend's complete size_t index count.
/// # Safety
/// A current context must have a live backend; a non-NULL buffer must meet its contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_indexbuffer_get_num_indices(buffer: *const c_void) -> usize {
    core::get_num_indices(&mut current(), optional(buffer))
}
