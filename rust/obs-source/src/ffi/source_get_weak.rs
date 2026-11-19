//! Borrow the source's control through a private C adapter and retain it
//! through the C-owned `obs_weak_source_addref`; release stays in C.
use crate::get_weak::{Host, get_weak_source};
use core::ffi::c_void;
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_source_get_weak_control(source: *mut c_void) -> *mut c_void;
    fn obs_weak_source_addref(weak: *mut c_void);
}

struct Libobs;

impl Host for Libobs {
    type Source = NonNull<c_void>;
    type Weak = NonNull<c_void>;

    fn control(&self, source: NonNull<c_void>) -> Option<NonNull<c_void>> {
        // SAFETY: the caller passes a live source; NULL was rejected first.
        NonNull::new(unsafe { obs_source_get_weak_control(source.as_ptr()) })
    }

    fn addref(&self, weak: Option<NonNull<c_void>>) {
        let weak = weak.map_or(core::ptr::null_mut(), NonNull::as_ptr);
        // SAFETY: the C addref accepts NULL; otherwise the control belongs to
        // the live source and its weak count is atomic.
        unsafe { obs_weak_source_addref(weak) }
    }
}

/// # Safety
/// A non-null source must be live under the original libobs ownership contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_weak_source(source: *mut c_void) -> *mut c_void {
    get_weak_source(&Libobs, NonNull::new(source)).map_or(core::ptr::null_mut(), NonNull::as_ptr)
}
