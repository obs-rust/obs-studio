//! Thin ABI bridge to the borrowed hidden-state field.

use core::ffi::c_void;

unsafe extern "C" {
    fn obs_source_hidden_setter_field(source: *mut c_void) -> *mut bool;
}

/// # Safety
/// A non-null source must be live and exclusively writable under the same
/// synchronization as the original C setter. NULL is outside the C contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_set_hidden(source: *mut c_void, hidden: bool) {
    if source.is_null() {
        return;
    }
    // SAFETY: NULL was checked before the adapter computes the field address.
    let field = unsafe { obs_source_hidden_setter_field(source) };
    // SAFETY: the caller exclusively owns this live, writable boolean field.
    crate::hidden_setter::set_hidden(unsafe { &mut *field }, hidden);
}
