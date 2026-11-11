//! Opaque vertex-buffer ABI; C owns private layouts, callbacks and logging.

use std::ffi::{CStr, c_char, c_void};
use std::ptr::NonNull;

use crate::vertexbuffer_access::{self, Backend, Error};

unsafe extern "C" {
    fn obs_vertexbuffer_access_context() -> *mut c_void;
    fn obs_vertexbuffer_access_flush(context: *mut c_void, buffer: *mut c_void);
    fn obs_vertexbuffer_access_get_data(context: *mut c_void, buffer: *const c_void)
    -> *mut c_void;
    fn obs_vertexbuffer_access_error(function: *const c_char, no_context: bool);
}

struct Context(NonNull<c_void>);

impl Backend<NonNull<c_void>, *mut c_void> for Context {
    fn flush(&mut self, buffer: NonNull<c_void>) {
        // SAFETY: the wrapper borrows the current context and live opaque buffer
        // only for this synchronous call; C dispatches its device callback.
        unsafe { obs_vertexbuffer_access_flush(self.0.as_ptr(), buffer.as_ptr()) };
    }

    fn get_data(&mut self, buffer: NonNull<c_void>) -> *mut c_void {
        // SAFETY: same synchronous borrow as flush; the opaque result is not read.
        unsafe { obs_vertexbuffer_access_get_data(self.0.as_ptr(), buffer.as_ptr()) }
    }
}

fn report(function: &CStr, error: Error) {
    // SAFETY: function is a static C string; C consumes it without retaining it.
    unsafe { obs_vertexbuffer_access_error(function.as_ptr(), error == Error::NoContext) };
}

/// Flush the current device's buffer.
///
/// # Safety
/// A nonnull buffer must belong to the live current graphics context.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_vertexbuffer_flush(buffer: *mut c_void) {
    // SAFETY: context lookup reads thread-local state and retains no pointer.
    let mut context = NonNull::new(unsafe { obs_vertexbuffer_access_context() }).map(Context);
    if let Err(error) = vertexbuffer_access::flush(context.as_mut(), NonNull::new(buffer)) {
        report(c"gs_vertexbuffer_flush", error);
    }
}

/// Return the device's opaque data pointer unchanged.
///
/// # Safety
/// A nonnull buffer must belong to the live current graphics context.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_vertexbuffer_get_data(buffer: *const c_void) -> *mut c_void {
    // SAFETY: context lookup reads thread-local state and retains no pointer.
    let mut context = NonNull::new(unsafe { obs_vertexbuffer_access_context() }).map(Context);
    vertexbuffer_access::get_data(context.as_mut(), NonNull::new(buffer.cast_mut())).unwrap_or_else(
        |error| {
            report(c"gs_vertexbuffer_get_data", error);
            std::ptr::null_mut()
        },
    )
}
