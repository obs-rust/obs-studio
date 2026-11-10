//! Registry lookup and borrowed field conversion; private C layout stays in C.
use crate::source_icon_type;
use core::ffi::{c_char, c_uint, c_void};

unsafe extern "C" {
    fn get_source_info(id: *const c_char) -> *const c_void;
    fn obs_source_icon_type_fields(info: *const c_void) -> *const c_uint;
}

/// # Safety
/// OBS must be initialized and `id` must satisfy get_source_info's original
/// C contract (a readable, NUL-terminated string when the registry is nonempty).
/// The registry entry must remain live throughout this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_icon_type(id: *const c_char) -> c_uint {
    // SAFETY: the caller supplies the same registry/string lifetime as C.
    let info = unsafe { get_source_info(id) };
    let registered = if info.is_null() {
        None
    } else {
        // SAFETY: reject NULL before forming a C field address; the registry
        // owns a live entry, and the C enum has c_uint size/alignment (tested).
        Some(unsafe { *obs_source_icon_type_fields(info) })
    };
    source_icon_type::icon_type(registered)
}
