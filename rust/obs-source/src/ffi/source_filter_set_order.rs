//! `obs_source_filter_set_order()`; the filter list, the flags, the
//! targets and the signal stay behind the field adapters of
//! `obs-source-filter-set-order-fields.c.h`.
use crate::filter_set_order::{FilterChain, set_order};
use core::ffi::{CStr, c_char, c_int, c_void};
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_source_filter_set_order_lock(source: *mut c_void);
    fn obs_source_filter_set_order_count(source: *const c_void) -> usize;
    fn obs_source_filter_set_order_at(source: *const c_void, index: usize) -> *mut c_void;
    fn obs_source_filter_set_order_flags(filter: *const c_void) -> u32;
    fn obs_source_filter_set_order_move(source: *mut c_void, from: usize, to: usize);
    fn obs_source_filter_set_order_target(filter: *mut c_void, target: *mut c_void);
    fn obs_source_filter_set_order_unlock(source: *mut c_void);
    fn obs_source_filter_set_order_signal(source: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// Constructed only for a live, non-NULL source.
struct Live {
    source: NonNull<c_void>,
}

impl FilterChain for Live {
    type Filter = *mut c_void;
    fn lock(&self) {
        // SAFETY: a live source owns an initialized filter_mutex.
        unsafe { obs_source_filter_set_order_lock(self.source.as_ptr()) }
    }
    fn count(&self) -> usize {
        // SAFETY: a live source, read under its filter_mutex.
        unsafe { obs_source_filter_set_order_count(self.source.as_ptr()) }
    }
    fn filter(&self, index: usize) -> *mut c_void {
        // SAFETY: the core asks only below a count read under the held
        // filter_mutex, the original C's contract for the array.
        unsafe { obs_source_filter_set_order_at(self.source.as_ptr(), index) }
    }
    fn flags(&self, filter: *mut c_void) -> u32 {
        // SAFETY: the caller's filter (found in the list before any read) or
        // an entry of the list, live filters under the original contract.
        unsafe { obs_source_filter_set_order_flags(filter) }
    }
    fn move_filter(&self, from: usize, to: usize) {
        // SAFETY: both indexes are below the count, under the held lock.
        unsafe { obs_source_filter_set_order_move(self.source.as_ptr(), from, to) }
    }
    fn set_target(&self, filter: *mut c_void, target: Option<*mut c_void>) {
        let target = target.unwrap_or(self.source.as_ptr());
        // SAFETY: an entry of the list, written under the held lock.
        unsafe { obs_source_filter_set_order_target(filter, target) }
    }
    fn unlock(&self) {
        // SAFETY: matches the lock the core took on this live source.
        unsafe { obs_source_filter_set_order_unlock(self.source.as_ptr()) }
    }
    fn signal(&self) {
        // SAFETY: a live source with its signal handler, after the unlock.
        unsafe { obs_source_filter_set_order_signal(self.source.as_ptr()) }
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
/// signal handler under the original libobs contract; a non-NULL filter is
/// read only once it is found in that list.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_filter_set_order(
    source: *mut c_void,
    filter: *mut c_void,
    movement: c_int,
) {
    // No adapter runs until NULL has been rejected.
    let live = NonNull::new(source).map(|source| Live { source });
    let filter = (!filter.is_null()).then_some(filter);
    set_order(live.as_ref(), filter, movement, null_log);
}
