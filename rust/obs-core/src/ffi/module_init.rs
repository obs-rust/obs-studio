//! C ABI bridge for module initialization; no private struct is mirrored.
use crate::module_init::{Lifecycle, initialize};
use std::ffi::{c_char, c_void};

/// Shared with the remaining C registration callers, serialized by OBS loading.
#[allow(non_upper_case_globals)] // Preserve the existing private C symbol.
#[unsafe(no_mangle)]
pub static mut loadingModule: *mut c_void = std::ptr::null_mut();

unsafe extern "C" {
    fn obs_module_init_loaded_field(module: *mut c_void) -> *mut bool;
    fn obs_module_init_load(module: *mut c_void) -> bool;
    fn obs_module_init_profile_start(module: *mut c_void) -> *const c_char;
    fn obs_module_init_warning(module: *mut c_void);
    fn obs_module_init_profile_end(name: *const c_char);
}
struct Module(*mut c_void);
impl Lifecycle for Module {
    type Profile = *const c_char;
    fn loaded(&self) -> bool {
        // SAFETY: this bridge is only constructed for a live module.
        unsafe { *obs_module_init_loaded_field(self.0) }
    }
    fn set_loaded(&mut self, loaded: bool) {
        // SAFETY: the caller serializes module loading and the field is live.
        unsafe {
            *obs_module_init_loaded_field(self.0) = loaded;
        }
    }
    fn profile_start(&mut self) -> Self::Profile {
        // SAFETY: module file storage and the profiler name store are live.
        unsafe { obs_module_init_profile_start(self.0) }
    }
    fn loading(&mut self, active: bool) {
        // SAFETY: OBS serializes access to its existing loading-module global.
        unsafe {
            loadingModule = if active { self.0 } else { std::ptr::null_mut() };
        }
    }
    fn load(&mut self) -> bool {
        // SAFETY: obs_open_module supplied a valid plugin callback.
        unsafe { obs_module_init_load(self.0) }
    }
    fn warning(&mut self) {
        // SAFETY: file storage remains live for the diagnostic.
        unsafe { obs_module_init_warning(self.0) }
    }
    fn profile_end(&mut self, profile: Self::Profile) {
        // SAFETY: this is the borrowed identity returned by profile_start.
        unsafe {
            obs_module_init_profile_end(profile);
        }
    }
}

/// Initialize a plugin once successfully; failed initialization can be retried.
///
/// # Safety
/// `module` is null or a live module opened by OBS. Initialization, registration,
/// core startup and shutdown must be serialized as in the C implementation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_init_module(module: *mut c_void) -> bool {
    if module.is_null() {
        return initialize::<Module>(false, None);
    }
    // SAFETY: the caller serializes core lifetime with initialization.
    let present = unsafe { super::core_initialized::obs_initialized() };
    initialize(present, Some(&mut Module(module)))
}
