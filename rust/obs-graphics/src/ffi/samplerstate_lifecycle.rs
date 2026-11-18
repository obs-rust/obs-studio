//! Thin lifecycle ABI; C owns graphics layouts, TLS and device objects.
use crate::samplerstate_lifecycle::{self as core, Entry, Host, Rejection};
use std::ffi::{c_char, c_void};
use std::ptr;

unsafe extern "C" {
    fn obs_graphics_samplerstate_context() -> *mut c_void;
    fn obs_graphics_samplerstate_debug(function: *const c_char, reason: *const c_char);
    fn obs_graphics_samplerstate_create(context: *mut c_void, info: *const c_void) -> *mut c_void;
    fn obs_graphics_samplerstate_destroy(context: *mut c_void, sampler: *mut c_void);
}
struct Context(*mut c_void);
fn current() -> Context {
    // SAFETY: reads the calling thread's opaque context without dereferencing it.
    Context(unsafe { obs_graphics_samplerstate_context() })
}
impl Host for Context {
    type Info = *const c_void;
    type Sampler = *mut c_void;
    fn has_context(&self) -> bool {
        !self.0.is_null()
    }
    fn debug(&mut self, entry: Entry, reason: Rejection) {
        // SAFETY: both strings are static and NUL-terminated.
        unsafe {
            obs_graphics_samplerstate_debug(entry.c_name().as_ptr(), reason.message().as_ptr())
        }
    }
    fn create(&mut self, info: *const c_void) -> Option<*mut c_void> {
        // SAFETY: the safe core checked context and info before C field access.
        // The caller supplies backend-compatible settings, forwarded unchanged.
        let result = unsafe { obs_graphics_samplerstate_create(self.0, info) };
        (!result.is_null()).then_some(result)
    }
    fn destroy(&mut self, sampler: *mut c_void) {
        // SAFETY: the safe core checked context and sampler before field access.
        // Ownership and validity remain the backend/caller's contract.
        unsafe { obs_graphics_samplerstate_destroy(self.0, sampler) }
    }
}
/// Forward settings to the current device without copying or allocating.
/// # Safety
/// A live context must provide the required backend callback. Non-NULL info
/// must satisfy that backend's contract; the returned sampler remains backend-owned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_samplerstate_create(info: *const c_void) -> *mut c_void {
    core::create(&mut current(), (!info.is_null()).then_some(info)).unwrap_or(ptr::null_mut())
}
/// Forward a sampler to the current backend without wrapper-side tracking.
/// # Safety
/// A live context must provide the required callback. A non-NULL sampler must
/// satisfy the backend's destruction contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_samplerstate_destroy(sampler: *mut c_void) {
    core::destroy(&mut current(), (!sampler.is_null()).then_some(sampler));
}
