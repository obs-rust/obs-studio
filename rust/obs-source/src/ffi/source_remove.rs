//! `obs_source_remove()`; the removed flag, the reference, the signals, the
//! canvas and private reads, the canvas removal and the release stay behind
//! the adapters of `obs-source-remove-fields.c.h`.
use crate::source_remove::{Source, remove};
use core::ffi::{CStr, c_char, c_int, c_void};
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_source_remove_removed(source: *const c_void) -> bool;
    fn obs_source_remove_get_ref(source: *mut c_void) -> *mut c_void;
    fn obs_source_remove_set_removed(source: *mut c_void);
    fn obs_source_remove_signal(source: *mut c_void);
    fn obs_source_remove_has_canvas(source: *const c_void) -> bool;
    fn obs_source_remove_private(source: *const c_void) -> bool;
    fn obs_source_remove_from_canvas(source: *mut c_void);
    fn obs_source_remove_release(source: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// Constructed only for a live, non-NULL source.
struct Live {
    source: NonNull<c_void>,
}

impl Source for Live {
    type Ref = NonNull<c_void>;
    fn removed(&self) -> bool {
        // SAFETY: a live source; one atomic load of its flag.
        unsafe { obs_source_remove_removed(self.source.as_ptr()) }
    }
    fn get_ref(&self) -> Option<NonNull<c_void>> {
        // SAFETY: a live source; obs_source_get_ref() returns NULL once its
        // last reference is gone.
        NonNull::new(unsafe { obs_source_remove_get_ref(self.source.as_ptr()) })
    }
    fn set_removed(&self, s: NonNull<c_void>) {
        // SAFETY: the reference just taken keeps the source alive.
        unsafe { obs_source_remove_set_removed(s.as_ptr()) }
    }
    fn signal(&self, s: NonNull<c_void>) {
        // SAFETY: the referenced source and its signal handler.
        unsafe { obs_source_remove_signal(s.as_ptr()) }
    }
    fn has_canvas(&self) -> bool {
        // SAFETY: the source passed in, read as the original reads it.
        unsafe { obs_source_remove_has_canvas(self.source.as_ptr()) }
    }
    fn is_private(&self) -> bool {
        // SAFETY: the source passed in, read as the original reads it.
        unsafe { obs_source_remove_private(self.source.as_ptr()) }
    }
    fn remove_from_canvas(&self, s: NonNull<c_void>) {
        // SAFETY: the referenced source; its canvas pointer was set.
        unsafe { obs_source_remove_from_canvas(s.as_ptr()) }
    }
    fn release(&self, s: NonNull<c_void>) {
        // SAFETY: the reference get_ref() took, released once.
        unsafe { obs_source_remove_release(s.as_ptr()) }
    }
}

fn null_log(function: &'static CStr, parameter: &'static CStr) {
    // SAFETY: static strings match the two %s arguments of the format.
    unsafe {
        blog(
            400,
            c"%s: Null '%s' parameter".as_ptr(),
            function.as_ptr(),
            parameter.as_ptr(),
        );
    }
}

/// # Safety
/// A non-NULL source must be a source under the original libobs contract:
/// alive while the call reads it, then kept alive by the reference it takes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_remove(source: *mut c_void) {
    // No adapter runs until NULL has been rejected.
    let live = NonNull::new(source).map(|source| Live { source });
    remove(live.as_ref(), null_log);
}
