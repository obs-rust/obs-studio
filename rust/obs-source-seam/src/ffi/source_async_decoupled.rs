//! Thin ABI adapter for the safe async decoupling sequence.
use core::ffi::{c_char, c_int, c_void};
use obs_source::async_decoupled::{self, Source};

unsafe extern "C" {
    fn obs_source_async_decoupled_store(source: *mut c_void, value: bool);
    fn obs_source_async_decoupled_lock(source: *mut c_void);
    fn obs_source_async_decoupled_invalidate(source: *mut c_void);
    fn obs_source_async_decoupled_unlock(source: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

struct Seam(*mut c_void);
impl Source for Seam {
    fn set_decoupled(&mut self, value: bool) {
        // SAFETY: the entry point checked this live source before constructing Seam.
        unsafe { obs_source_async_decoupled_store(self.0, value) };
    }
    fn lock_audio(&mut self) {
        // SAFETY: live source with an initialized audio mutex, as required by C.
        unsafe { obs_source_async_decoupled_lock(self.0) };
    }
    fn invalidate_timing(&mut self) {
        // SAFETY: the safe core holds the source's audio mutex.
        unsafe { obs_source_async_decoupled_invalidate(self.0) };
    }
    fn reset_audio(&mut self, timestamp: u64) {
        // SAFETY: live source, with its audio mutex held by the safe core.
        unsafe { super::source_audio_output::reset_audio_data(self.0, timestamp) };
    }
    fn unlock_audio(&mut self) {
        // SAFETY: this call balances the safe core's lock on the same mutex.
        unsafe { obs_source_async_decoupled_unlock(self.0) };
    }
}

/// # Safety
/// A non-NULL source must be live with an initialized audio mutex and buffers,
/// under the same caller synchronization contract as the original C setter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_set_async_decoupled(source: *mut c_void, decouple: bool) {
    let mut seam = (!source.is_null()).then_some(Seam(source));
    async_decoupled::set(seam.as_mut(), decouple, || {
        // SAFETY: the format and its static string argument are NUL terminated.
        unsafe {
            blog(
                400,
                c"%s: Null 'source' parameter".as_ptr(),
                c"obs_source_set_async_decoupled".as_ptr(),
            );
        }
    });
}
