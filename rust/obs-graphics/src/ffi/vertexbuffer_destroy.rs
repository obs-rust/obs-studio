//! C owns the context layout and device callback; the core decides dispatch.
use std::ffi::c_void;
use std::ptr::NonNull;

unsafe extern "C" {
    fn obs_graphics_vertexbuffer_destroy_context() -> *mut c_void;
    fn obs_graphics_vertexbuffer_destroy_dispatch(context: *mut c_void, buffer: *mut c_void);
}

/// Destroy a device-owned vertex buffer in the current graphics context.
///
/// # Safety
/// A non-null buffer must belong to the current device and be live until its
/// destroy callback runs. The current context must remain live during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_vertexbuffer_destroy(buffer: *mut c_void) {
    // SAFETY: the adapter validates the current thread's context and preserves
    // the original log, even when buffer is null. It does not access the buffer.
    let context = NonNull::new(unsafe { obs_graphics_vertexbuffer_destroy_context() });
    let callback = context.map(|context| {
        move |buffer: NonNull<c_void>| {
            // SAFETY: the core dispatches once, only for a non-null buffer and a
            // validated context. The caller guarantees device ownership and lifetime.
            unsafe {
                obs_graphics_vertexbuffer_destroy_dispatch(context.as_ptr(), buffer.as_ptr())
            };
        }
    });
    crate::vertexbuffer_destroy::destroy(NonNull::new(buffer), callback);
}
