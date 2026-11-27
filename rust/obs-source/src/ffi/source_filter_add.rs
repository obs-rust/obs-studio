//! `obs_source_filter_add()`; the lock, the filter list, the flags, the
//! reference, the links, the signals, the log lines and the callback stay
//! behind the adapters of `obs-source-filter-add-fields.c.h`.
use crate::filter_add::{FilterList, add};
use core::ffi::{CStr, c_char, c_int, c_void};
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_source_filter_add_lock(source: *mut c_void);
    fn obs_source_filter_add_count(source: *const c_void) -> usize;
    fn obs_source_filter_add_at(source: *const c_void, index: usize) -> *mut c_void;
    fn obs_source_filter_add_has_parent(filter: *const c_void) -> bool;
    fn obs_source_filter_add_warn_present();
    fn obs_source_filter_add_owns_info_id(source: *const c_void) -> bool;
    fn obs_source_filter_add_output_flags(source: *const c_void) -> u32;
    fn obs_source_filter_add_get_ref(filter: *mut c_void) -> *mut c_void;
    fn obs_source_filter_add_set_parent(filter: *mut c_void, source: *mut c_void);
    fn obs_source_filter_add_set_target(filter: *mut c_void, target: *mut c_void);
    fn obs_source_filter_add_insert_front(source: *mut c_void, filter: *mut c_void);
    fn obs_source_filter_add_unlock(source: *mut c_void);
    fn obs_source_filter_add_signal(source: *mut c_void, filter: *mut c_void);
    fn obs_source_filter_add_log(source: *mut c_void, filter: *mut c_void);
    fn obs_source_filter_add_has_callback(filter: *const c_void) -> bool;
    fn obs_source_filter_add_callback(filter: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// Constructed only for a live, non-NULL source.
struct Live {
    source: NonNull<c_void>,
}

impl FilterList for Live {
    type Source = *mut c_void;
    fn this(&self) -> *mut c_void {
        self.source.as_ptr()
    }
    fn lock(&self) {
        // SAFETY: a live source owns an initialized filter_mutex.
        unsafe { obs_source_filter_add_lock(self.source.as_ptr()) }
    }
    fn count(&self) -> usize {
        // SAFETY: a live source, read under its filter_mutex.
        unsafe { obs_source_filter_add_count(self.source.as_ptr()) }
    }
    fn filter(&self, index: usize) -> *mut c_void {
        // SAFETY: the core asks only below a count read under the held
        // filter_mutex, the original C's contract for the array.
        unsafe { obs_source_filter_add_at(self.source.as_ptr(), index) }
    }
    fn has_parent(&self, filter: *mut c_void) -> bool {
        // SAFETY: the caller's live, non-NULL filter (the original read the
        // same field).
        unsafe { obs_source_filter_add_has_parent(filter) }
    }
    fn warn_present(&self) {
        // SAFETY: a fixed message with no arguments.
        unsafe { obs_source_filter_add_warn_present() }
    }
    fn owns_info_id(&self) -> bool {
        // SAFETY: a live source.
        unsafe { obs_source_filter_add_owns_info_id(self.source.as_ptr()) }
    }
    fn output_flags(&self) -> u32 {
        // SAFETY: a live source.
        unsafe { obs_source_filter_add_output_flags(self.source.as_ptr()) }
    }
    fn filter_output_flags(&self, filter: *mut c_void) -> u32 {
        // SAFETY: the caller's live, non-NULL filter.
        unsafe { obs_source_filter_add_output_flags(filter) }
    }
    fn get_ref(&self, filter: *mut c_void) -> Option<*mut c_void> {
        // SAFETY: the caller's non-NULL filter, whose control block outlives
        // its last strong reference (obs_source_get_ref's contract).
        let referenced = unsafe { obs_source_filter_add_get_ref(filter) };
        (!referenced.is_null()).then_some(referenced)
    }
    fn set_parent(&self, filter: *mut c_void) {
        // SAFETY: the referenced filter, written under the held lock.
        unsafe { obs_source_filter_add_set_parent(filter, self.source.as_ptr()) }
    }
    fn set_target(&self, filter: *mut c_void, target: *mut c_void) {
        // SAFETY: the referenced filter, written under the held lock.
        unsafe { obs_source_filter_add_set_target(filter, target) }
    }
    fn insert_front(&self, filter: *mut c_void) {
        // SAFETY: a live source's list, grown under the held lock; the list
        // takes over the reference the core holds.
        unsafe { obs_source_filter_add_insert_front(self.source.as_ptr(), filter) }
    }
    fn unlock(&self) {
        // SAFETY: matches the lock the core took on this live source.
        unsafe { obs_source_filter_add_unlock(self.source.as_ptr()) }
    }
    fn signal(&self, filter: *mut c_void) {
        // SAFETY: a live source with its signal handler, after the unlock;
        // the filter is only stored in the calldata.
        unsafe { obs_source_filter_add_signal(self.source.as_ptr(), filter) }
    }
    fn log_added(&self, filter: *mut c_void) {
        // SAFETY: the added filter, alive (the list's reference).
        unsafe { obs_source_filter_add_log(self.source.as_ptr(), filter) }
    }
    fn has_add_callback(&self, filter: *mut c_void) -> bool {
        // SAFETY: the added filter, alive.
        unsafe { obs_source_filter_add_has_callback(filter) }
    }
    fn call_add_callback(&self, filter: *mut c_void) {
        // SAFETY: the added filter, whose type has the callback.
        unsafe { obs_source_filter_add_callback(filter) }
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
/// must be a source whose control block is alive.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_filter_add(source: *mut c_void, filter: *mut c_void) {
    // No adapter runs until NULL has been rejected.
    let live = NonNull::new(source).map(|source| Live { source });
    let filter = (!filter.is_null()).then_some(filter);
    add(live.as_ref(), filter, null_log);
}
