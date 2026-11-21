//! Thin ABI over safe admission and the existing native device-query seam.
use crate::device_identity::{self as core, Entry, Host};
use std::ffi::{c_char, c_void};

unsafe extern "C" {
    fn obs_graphics_device_queries_context() -> *mut c_void;
    fn obs_graphics_device_queries_debug(function: *const c_char);
    fn obs_graphics_device_queries_object(context: *mut c_void) -> *mut c_void;
    fn obs_graphics_device_queries_name(context: *mut c_void) -> *const c_char;
}
struct Context(*mut c_void);
fn current() -> Context {
    // SAFETY: reads thread-local context without projecting any fields.
    Context(unsafe { obs_graphics_device_queries_context() })
}
impl Host for Context {
    type Value = *const c_void;
    fn has_context(&self) -> bool {
        !self.0.is_null()
    }
    fn debug(&mut self, entry: Entry) {
        let name = match entry {
            Entry::Object => c"gs_get_device_obj",
            Entry::Name => c"gs_get_device_name",
        };
        // SAFETY: static NUL-terminated name, passed to the original diagnostic.
        unsafe { obs_graphics_device_queries_debug(name.as_ptr()) }
    }
    fn dispatch(&mut self, entry: Entry) -> Self::Value {
        // SAFETY: safe core checked context before field projection. Both
        // callbacks are required in a live context; returned pointers are borrowed.
        unsafe {
            match entry {
                Entry::Object => obs_graphics_device_queries_object(self.0).cast_const(),
                Entry::Name => obs_graphics_device_queries_name(self.0).cast(),
            }
        }
    }
}
/// Borrow the backend device object, or NULL without a context.
/// # Safety
/// Any current context and its mandatory backend callback must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_get_device_obj() -> *mut c_void {
    core::identity(&mut current(), Entry::Object).cast_mut()
}
/// Borrow the backend name, or NULL without a context.
/// # Safety
/// Any current context and its mandatory backend callback must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_get_device_name() -> *const c_char {
    core::identity(&mut current(), Entry::Name).cast()
}
