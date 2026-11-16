//! Opaque C item boundary: validate before obtaining any private field address.
use core::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_sceneitem_blending_method_field(item: *mut c_void) -> *mut u32;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// Store the caller's enum bits without changing transform state.
///
/// # Safety
/// A non-null item must be live and exclusively writable under the original C
/// setter's synchronization contract for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_set_blending_method(item: *mut c_void, method: u32) {
    let field = if item.is_null() {
        None
    } else {
        // SAFETY: NULL was rejected; the real C adapter returns the live enum
        // field only. Its size and alignment match u32, as asserted in C.
        Some(unsafe { &mut *obs_sceneitem_blending_method_field(item) })
    };
    crate::item_blending_method::set_method(field, method, || {
        // SAFETY: the format and its arguments are static terminated C strings.
        unsafe {
            blog(
                400,
                c"%s: Null '%s' parameter".as_ptr(),
                c"obs_sceneitem_set_blending_method".as_ptr(),
                c"item".as_ptr(),
            );
        }
    });
}
