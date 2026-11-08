//! Thin C ABI read of the private global pointer slot.
use crate::core_initialized::initialized;
use std::ffi::c_void;

unsafe extern "C" {
    fn obs_core_initialized_field() -> *const *const c_void;
}

/// Report whether the core pointer has been set.
///
/// # Safety
/// Core startup and shutdown must not mutate the global pointer concurrently.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_initialized() -> bool {
    // SAFETY: the adapter returns the address of the live global pointer slot,
    // even before startup or after shutdown. Its pointee is never dereferenced.
    unsafe { initialized(*obs_core_initialized_field()) }
}
