//! Borrow the existing C callback record; leave the rest of the output opaque.
use core::ffi::{c_char, c_int, c_void};

pub type Callback = unsafe extern "C" fn(*mut c_void, *mut c_void, c_int) -> bool;

#[repr(C)]
pub struct ReconnectCallback {
    pub reconnect_cb: Option<Callback>,
    pub param: *mut c_void,
}

unsafe extern "C" {
    fn obs_output_reconnect_callback_fields(output: *mut c_void) -> *mut ReconnectCallback;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// # Safety
/// A non-NULL `output` must be live, with exclusive access to its reconnect
/// callback record. Callback and data must remain valid when invoked by OBS.
/// A NULL output logs `obs_output_valid`'s debug line and returns (#4754).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_set_reconnect_callback(
    output: *mut c_void,
    callback: Option<Callback>,
    param: *mut c_void,
) {
    if !crate::output_reconnect_callback::accepts_output(!output.is_null()) {
        // SAFETY: static printf strings, one string argument.
        unsafe {
            blog(
                400,
                c"%s: Null 'output' parameter".as_ptr(),
                c"obs_output_set_reconnect_callback".as_ptr(),
            )
        };
        return;
    }
    // SAFETY: caller supplies a live non-null output and exclusive record access.
    let fields = unsafe { &mut *obs_output_reconnect_callback_fields(output) };
    (fields.reconnect_cb, fields.param) = crate::output_reconnect_callback::select(callback, param);
}
