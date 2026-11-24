//! The source layout stays behind the flag address adapter of
//! `obs-source-removed-fields.c.h`; the shim loads the C `volatile bool`
//! atomically itself, as `os_atomic_load_bool()` does.
use crate::removed::removed;
use core::ffi::{CStr, c_char, c_int, c_void};
use core::sync::atomic::{AtomicBool, Ordering};

unsafe extern "C" {
    fn obs_source_removed_field(source: *const c_void) -> *const bool;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// # Safety
/// A non-null source must be live under the original libobs ownership contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_removed(source: *const c_void) -> bool {
    let load = (!source.is_null()).then_some(|| {
        // SAFETY: NULL was rejected before the private C layout accessor; the
        // adapter returns the address of the live source's flag, a C bool,
        // which has the size and alignment of AtomicBool.
        unsafe { (*obs_source_removed_field(source).cast::<AtomicBool>()).load(Ordering::SeqCst) }
    });
    removed(load, |name: &'static CStr| {
        // SAFETY: static strings match the two %s arguments of the format.
        unsafe {
            blog(
                400,
                c"%s: Null '%s' parameter".as_ptr(),
                name.as_ptr(),
                c"source".as_ptr(),
            );
        }
    })
}
