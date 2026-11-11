//! Private encoder layout stays in C; the byte comparison belongs to the safe core.
use crate::encoder_set_name::should_rename;
use core::ffi::{CStr, c_char, c_int, c_void};

#[repr(C)]
pub struct SetNameFields {
    pub context: *mut c_void,
    pub name: *const c_char,
}

unsafe extern "C" {
    fn obs_encoder_set_name_fields(encoder: *mut c_void) -> SetNameFields;
    fn obs_context_data_setname(context: *mut c_void, name: *const c_char);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// # Safety
/// A non-null encoder must be live, with a valid context name. A non-null name
/// must be NUL-terminated. The caller provides the original setter's synchronization.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_set_name(encoder: *mut c_void, name: *const c_char) {
    if encoder.is_null() {
        // SAFETY: static format without conversion arguments.
        unsafe {
            blog(
                400,
                c"obs_encoder_set_name: Null 'encoder' parameter".as_ptr(),
            )
        };
        return;
    }
    // SAFETY: NULL was checked before taking private field addresses.
    let fields = unsafe { obs_encoder_set_name_fields(encoder) };
    // SAFETY: the caller guarantees live NUL-terminated strings. No borrow survives rename.
    let rename = unsafe {
        should_rename(
            || CStr::from_ptr(fields.name).to_bytes(),
            name.as_ref().map(|_| CStr::from_ptr(name).to_bytes()),
        )
    };
    if rename {
        // SAFETY: original name pointer and real context are valid through this call.
        unsafe { obs_context_data_setname(fields.context, name) };
    }
}
