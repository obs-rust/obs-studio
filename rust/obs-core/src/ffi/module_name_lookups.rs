//! Borrowed opaque registry traversal; private layouts stay in C.
use std::ffi::{CStr, c_char, c_void};
use std::ptr::NonNull;

unsafe extern "C" {
    fn obs_module_name_lookup_head_field(disabled: bool) -> *const *mut c_void;
    fn obs_module_name_lookup_name_field(module: *const c_void) -> *const *const c_char;
    fn obs_module_name_lookup_next_field(module: *const c_void) -> *const *mut c_void;
}

unsafe fn lookup(name: *const c_char, disabled: bool) -> *mut c_void {
    // SAFETY: caller guarantees a terminated input and synchronized live registry.
    unsafe {
        let first = obs_module_name_lookup_head_field(disabled)
            .as_ref()
            .copied()
            .and_then(NonNull::new);
        crate::module_name_lookups::lookup(first, CStr::from_ptr(name).to_bytes(), |module| {
            let module = module.as_ptr();
            let candidate = *obs_module_name_lookup_name_field(module);
            let next = *obs_module_name_lookup_next_field(module);
            (CStr::from_ptr(candidate).to_bytes(), NonNull::new(next))
        })
        .map_or(std::ptr::null_mut(), NonNull::as_ptr)
    }
}

/// Find the first loaded module with an exactly matching name.
///
/// # Safety
/// OBS is initialized, `name` is a non-null terminated byte string, and the
/// module registry is live and synchronized for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_module(name: *const c_char) -> *mut c_void {
    // SAFETY: forwarded registry and string requirements are the caller's contract.
    unsafe { lookup(name, false) }
}

/// Find the first disabled module with an exactly matching name.
///
/// # Safety
/// Same requirements as `obs_get_module`; the returned handle remains borrowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_disabled_module(name: *const c_char) -> *mut c_void {
    // SAFETY: forwarded registry and string requirements are the caller's contract.
    unsafe { lookup(name, true) }
}
