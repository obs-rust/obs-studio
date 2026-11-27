//! `obs_source_filter_remove()` and the hidden
//! `obs_source_filter_remove_refless()` that `obs_source_destroy()` calls;
//! the lock, the filter list, the targets, the signals, the log line, the
//! callback and the release stay behind the adapters of
//! `obs-source-filter-remove-fields.c.h`.
use crate::filter_remove::{FilterList, remove, remove_refless};
use core::ffi::{CStr, c_char, c_int, c_void};
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_source_filter_remove_lock(source: *mut c_void);
    fn obs_source_filter_remove_count(source: *const c_void) -> usize;
    fn obs_source_filter_remove_at(source: *const c_void, index: usize) -> *mut c_void;
    fn obs_source_filter_remove_target(filter: *const c_void) -> *mut c_void;
    fn obs_source_filter_remove_set_target(filter: *mut c_void, target: *mut c_void);
    fn obs_source_filter_remove_erase(source: *mut c_void, index: usize);
    fn obs_source_filter_remove_unlock(source: *mut c_void);
    fn obs_source_filter_remove_signal(source: *mut c_void, filter: *mut c_void);
    fn obs_source_filter_remove_log(source: *mut c_void, filter: *mut c_void);
    fn obs_source_filter_remove_has_callback(filter: *const c_void) -> bool;
    fn obs_source_filter_remove_callback(filter: *mut c_void);
    fn obs_source_filter_remove_clear_links(filter: *mut c_void);
    fn obs_source_filter_remove_release(filter: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// Constructed only for a live, non-NULL source.
struct Live {
    source: NonNull<c_void>,
}

impl FilterList for Live {
    type Source = *mut c_void;
    fn lock(&self) {
        // SAFETY: a live source owns an initialized filter_mutex.
        unsafe { obs_source_filter_remove_lock(self.source.as_ptr()) }
    }
    fn count(&self) -> usize {
        // SAFETY: a live source, read under its filter_mutex.
        unsafe { obs_source_filter_remove_count(self.source.as_ptr()) }
    }
    fn filter(&self, index: usize) -> *mut c_void {
        // SAFETY: the core asks only below a count read under the held
        // filter_mutex, the original C's contract for the array.
        unsafe { obs_source_filter_remove_at(self.source.as_ptr(), index) }
    }
    fn target(&self, filter: *mut c_void) -> *mut c_void {
        // SAFETY: the caller's filter, read only once found on the list,
        // under the held lock.
        unsafe { obs_source_filter_remove_target(filter) }
    }
    fn set_target(&self, filter: *mut c_void, target: *mut c_void) {
        // SAFETY: an entry of the list, written under the held lock.
        unsafe { obs_source_filter_remove_set_target(filter, target) }
    }
    fn erase(&self, index: usize) {
        // SAFETY: an index below the count, under the held lock.
        unsafe { obs_source_filter_remove_erase(self.source.as_ptr(), index) }
    }
    fn unlock(&self) {
        // SAFETY: matches the lock the core took on this live source.
        unsafe { obs_source_filter_remove_unlock(self.source.as_ptr()) }
    }
    fn signal(&self, filter: *mut c_void) {
        // SAFETY: a live source with its signal handler, after the unlock;
        // the filter is only stored in the calldata.
        unsafe { obs_source_filter_remove_signal(self.source.as_ptr(), filter) }
    }
    fn log_removed(&self, filter: *mut c_void) {
        // SAFETY: the removed filter, still alive (the caller's reference).
        unsafe { obs_source_filter_remove_log(self.source.as_ptr(), filter) }
    }
    fn has_remove_callback(&self, filter: *mut c_void) -> bool {
        // SAFETY: the removed filter, still alive.
        unsafe { obs_source_filter_remove_has_callback(filter) }
    }
    fn call_remove_callback(&self, filter: *mut c_void) {
        // SAFETY: the removed filter, whose type has the callback.
        unsafe { obs_source_filter_remove_callback(filter) }
    }
    fn clear_links(&self, filter: *mut c_void) {
        // SAFETY: the removed filter, still alive.
        unsafe { obs_source_filter_remove_clear_links(filter) }
    }
    fn release(&self, filter: *mut c_void) {
        // SAFETY: the list's reference to the removed filter, released once.
        unsafe { obs_source_filter_remove_release(filter) }
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
/// A non-NULL source must be live with an initialized filter list and
/// signal handler under the original libobs contract; a non-NULL filter
/// must be live, and is followed only once it is found on that list.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_filter_remove(source: *mut c_void, filter: *mut c_void) {
    // No adapter runs until NULL has been rejected.
    let live = NonNull::new(source).map(|source| Live { source });
    let filter = (!filter.is_null()).then_some(filter);
    remove(live.as_ref(), filter, null_log);
}

/// # Safety
/// As [`obs_source_filter_remove`]; `obs_source_destroy()` passes a filter's
/// parent and the filter, never NULL. A NULL source (which the original
/// would follow) returns `false` without a call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_filter_remove_refless(
    source: *mut c_void,
    filter: *mut c_void,
) -> bool {
    NonNull::new(source).is_some_and(|source| remove_refless(&Live { source }, filter))
}
