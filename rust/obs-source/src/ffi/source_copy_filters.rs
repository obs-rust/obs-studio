//! `obs_source_copy_filters()`, `obs_source_copy_single_filter()` and the
//! hidden `obs_source_duplicate_filters()` that `obs_source_duplicate()`
//! calls; the lock, the filter list, the naming, the compatibility check
//! and every libobs call stay behind the adapters of
//! `obs-source-copy-filters-fields.c.h`.
use crate::copy_filters::{FilterCopy, copy_filters, copy_single_filter, duplicate_filters};
use core::ffi::{CStr, c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_source_copy_filters_private(dst: *const c_void) -> bool;
    fn obs_source_copy_filters_lock(src: *mut c_void);
    fn obs_source_copy_filters_count(src: *const c_void) -> usize;
    fn obs_source_copy_filters_at(src: *const c_void, index: usize) -> *mut c_void;
    fn obs_source_copy_filters_get_ref(filter: *mut c_void) -> *mut c_void;
    fn obs_source_copy_filters_unlock(src: *mut c_void);
    fn obs_source_copy_filters_compatible(dst: *mut c_void, filter: *mut c_void) -> bool;
    fn obs_source_copy_filters_new_name(dst: *mut c_void, filter: *mut c_void) -> *mut c_char;
    fn obs_source_copy_filters_enabled(filter: *const c_void) -> bool;
    fn obs_source_copy_filters_duplicate(
        filter: *mut c_void,
        name: *const c_char,
        private: bool,
    ) -> *mut c_void;
    fn obs_source_copy_filters_set_enabled(copy: *mut c_void, enabled: bool);
    fn obs_source_copy_filters_free_name(name: *mut c_char);
    fn obs_source_copy_filters_add(dst: *mut c_void, copy: *mut c_void);
    fn obs_source_copy_filters_release(source: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// The live libobs: every handle is passed to the adapters as the original
/// C passed it, so a NULL duplicate reaches the same calls.
struct Libobs;

impl FilterCopy for Libobs {
    type Source = *mut c_void;
    type Name = *mut c_char;
    fn private(&self, dst: *mut c_void) -> bool {
        // SAFETY: a live destination, checked non-NULL by the core.
        unsafe { obs_source_copy_filters_private(dst) }
    }
    fn lock(&self, src: *mut c_void) {
        // SAFETY: a live source owns an initialized filter_mutex.
        unsafe { obs_source_copy_filters_lock(src) }
    }
    fn count(&self, src: *mut c_void) -> usize {
        // SAFETY: a live source, read under its filter_mutex.
        unsafe { obs_source_copy_filters_count(src) }
    }
    fn filter(&self, src: *mut c_void, index: usize) -> *mut c_void {
        // SAFETY: the core asks only below a count read under the held
        // filter_mutex, the original C's contract for the array.
        unsafe { obs_source_copy_filters_at(src, index) }
    }
    fn get_ref(&self, filter: *mut c_void) -> Option<*mut c_void> {
        // SAFETY: an entry of the list, under the held lock.
        let filter = unsafe { obs_source_copy_filters_get_ref(filter) };
        (!filter.is_null()).then_some(filter)
    }
    fn unlock(&self, src: *mut c_void) {
        // SAFETY: matches the lock the core took on this live source.
        unsafe { obs_source_copy_filters_unlock(src) }
    }
    fn compatible(&self, dst: *mut c_void, filter: *mut c_void) -> bool {
        // SAFETY: both live and non-NULL, checked by the core.
        unsafe { obs_source_copy_filters_compatible(dst, filter) }
    }
    fn new_name(&self, dst: *mut c_void, filter: *mut c_void) -> *mut c_char {
        // SAFETY: a live filter (referenced or the caller's); the
        // destination is passed on as the original passed it.
        unsafe { obs_source_copy_filters_new_name(dst, filter) }
    }
    fn enabled(&self, filter: *mut c_void) -> bool {
        // SAFETY: a live filter.
        unsafe { obs_source_copy_filters_enabled(filter) }
    }
    fn duplicate(&self, filter: *mut c_void, name: &*mut c_char, private: bool) -> *mut c_void {
        // SAFETY: a live filter and the name new_name() returned, not yet
        // freed.
        unsafe { obs_source_copy_filters_duplicate(filter, *name, private) }
    }
    fn set_enabled(&self, copy: *mut c_void, enabled: bool) {
        // SAFETY: the duplicate as returned, NULL included, which the
        // setter checks.
        unsafe { obs_source_copy_filters_set_enabled(copy, enabled) }
    }
    fn free_name(&self, name: *mut c_char) {
        // SAFETY: the name new_name() returned, freed once.
        unsafe { obs_source_copy_filters_free_name(name) }
    }
    fn add(&self, dst: *mut c_void, copy: *mut c_void) {
        // SAFETY: the destination and the duplicate as the original passed
        // them; obs_source_filter_add() checks both.
        unsafe { obs_source_copy_filters_add(dst, copy) }
    }
    fn release(&self, source: *mut c_void) {
        // SAFETY: a reference this call owns (the duplicate, NULL included,
        // or the snapshot's).
        unsafe { obs_source_copy_filters_release(source) }
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
/// Non-NULL sources must be live under the original libobs contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_copy_filters(dst: *mut c_void, src: *mut c_void) {
    // No adapter runs until NULL has been rejected.
    let dst = (!dst.is_null()).then_some(dst);
    let src = (!src.is_null()).then_some(src);
    copy_filters(&Libobs, dst, src, null_log);
}

/// # Safety
/// Non-NULL sources must be live under the original libobs contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_copy_single_filter(dst: *mut c_void, filter: *mut c_void) {
    // No adapter runs until NULL has been rejected.
    let dst = (!dst.is_null()).then_some(dst);
    let filter = (!filter.is_null()).then_some(filter);
    copy_single_filter(&Libobs, dst, filter, null_log);
}

/// # Safety
/// `src` must be a live source; `dst` is passed on to the naming and add
/// calls as `obs_source_duplicate()` gives it (never NULL there).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_duplicate_filters(
    dst: *mut c_void,
    src: *mut c_void,
    private: bool,
) {
    duplicate_filters(&Libobs, dst, src, private);
}
