//! Borrow monitoring fields through private typed adapters, keeping core layout in C.
use crate::monitoring_device_getter::{Field, get};
use std::ffi::{c_char, c_void};

unsafe extern "C" {
    fn obs_core_initialized_field() -> *const *const c_void;
    fn obs_core_monitoring_device_name(core: *const c_void) -> *const *mut c_char;
    fn obs_core_monitoring_device_id(core: *const c_void) -> *const *mut c_char;
}

/// Return the borrowed monitoring name and ID through independently optional outputs.
///
/// # Safety
/// Non-null outputs must be writable pointer slots. The core must be initialized
/// for requested fields and its monitoring configuration must not change concurrently.
/// Outputs may alias each other; their writes occur in name, then ID order.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_audio_monitoring_device(
    name: *mut *const c_char,
    id: *mut *const c_char,
) {
    // SAFETY: the existing adapter returns the live global pointer slot.
    let core = unsafe { *obs_core_initialized_field() };
    if core.is_null() {
        return;
    }
    let read = |field| {
        // SAFETY: core is non-null and live by the caller contract. The field
        // adapters return live pointer slots, without transferring ownership.
        unsafe {
            match field {
                Field::Name => (*obs_core_monitoring_device_name(core)).cast_const(),
                Field::Id => (*obs_core_monitoring_device_id(core)).cast_const(),
            }
        }
    };
    // SAFETY: each non-null output is writable. Separate calls end the first
    // mutable borrow before creating the second, preserving aliased C outputs.
    unsafe { get(name.as_mut(), None, read) };
    // SAFETY: same output contract; the earlier mutable borrow has ended.
    unsafe { get(None, id.as_mut(), read) };
}
