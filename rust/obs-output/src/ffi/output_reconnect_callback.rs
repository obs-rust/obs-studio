//! Borrow the existing C callback record; leave the rest of the output opaque.
use core::ffi::{c_int, c_void};

pub type Callback = unsafe extern "C" fn(*mut c_void, *mut c_void, c_int) -> bool;

#[repr(C)]
pub struct ReconnectCallback {
    pub reconnect_cb: Option<Callback>,
    pub param: *mut c_void,
}

unsafe extern "C" {
    fn obs_output_reconnect_callback_fields(output: *mut c_void) -> *mut ReconnectCallback;
}

/// # Safety
/// `output` must be a live, non-null output with exclusive access to its
/// reconnect callback record. Callback and data must remain valid when invoked
/// by OBS. This retains the original setter's non-null output precondition.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_set_reconnect_callback(
    output: *mut c_void,
    callback: Option<Callback>,
    param: *mut c_void,
) {
    assert!(!output.is_null(), "output must not be NULL");
    // SAFETY: caller supplies a live non-null output and exclusive record access.
    let fields = unsafe { &mut *obs_output_reconnect_callback_fields(output) };
    (fields.reconnect_cb, fields.param) = crate::output_reconnect_callback::select(callback, param);
}
