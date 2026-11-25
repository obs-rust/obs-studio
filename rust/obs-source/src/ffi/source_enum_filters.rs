//! The filter list stays behind the field adapters of
//! `obs-source-enum-filters-fields.c.h`.
use crate::enum_filters::{FilterList, enum_filters};
use core::ffi::{CStr, c_char, c_int, c_void};
use core::ptr::NonNull;

/// `obs_source_enum_proc_t`.
pub type EnumProc = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void);

unsafe extern "C" {
    fn obs_source_enum_filters_lock(source: *mut c_void);
    fn obs_source_enum_filters_count(source: *const c_void) -> usize;
    fn obs_source_enum_filters_at(source: *const c_void, index: usize) -> *mut c_void;
    fn obs_source_enum_filters_unlock(source: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// Constructed only for a live, non-NULL source.
struct Live {
    source: NonNull<c_void>,
}

impl FilterList for Live {
    type Filter = *mut c_void;
    fn lock(&self) {
        // SAFETY: a live source owns an initialized filter_mutex.
        unsafe { obs_source_enum_filters_lock(self.source.as_ptr()) }
    }
    fn count(&self) -> usize {
        // SAFETY: a live source, read under its filter_mutex.
        unsafe { obs_source_enum_filters_count(self.source.as_ptr()) }
    }
    fn filter(&self, index: usize) -> *mut c_void {
        // SAFETY: the core asks only below a count read under the held
        // filter_mutex, the original C's contract for the array.
        unsafe { obs_source_enum_filters_at(self.source.as_ptr(), index) }
    }
    fn unlock(&self) {
        // SAFETY: matches the lock the core took on this live source.
        unsafe { obs_source_enum_filters_unlock(self.source.as_ptr()) }
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
/// A non-NULL source must be live with an initialized filter list under the
/// original libobs contract; a non-NULL callback is called with the list
/// locked, as the C did.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_enum_filters(
    source: *mut c_void,
    callback: Option<EnumProc>,
    param: *mut c_void,
) {
    // No adapter runs until NULL has been rejected.
    let live = NonNull::new(source).map(|source| Live { source });
    let callback = callback.map(|callback| {
        move |filter: *mut c_void| {
            // SAFETY: the caller's callback, with the arguments the C passed.
            unsafe { callback(source, filter, param) }
        }
    });
    enum_filters(live.as_ref(), callback, null_log);
}
