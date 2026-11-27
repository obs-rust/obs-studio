//! `obs_source_restore_filters()`; the lock, the filter list, the links,
//! the array reads, the references, the loads and the releases stay behind
//! the adapters of `obs-source-restore-filters-fields.c.h`.
use crate::restore_filters::{RestoreList, restore};
use core::ffi::{CStr, c_char, c_int, c_void};
use core::ptr::{NonNull, null_mut};

unsafe extern "C" {
    fn obs_source_restore_filters_lock(source: *mut c_void);
    fn obs_source_restore_filters_count(source: *const c_void) -> usize;
    fn obs_source_restore_filters_at(source: *const c_void, index: usize) -> *mut c_void;
    fn obs_source_restore_filters_set_parent(filter: *mut c_void, parent: *mut c_void);
    fn obs_source_restore_filters_set_target(filter: *mut c_void, target: *mut c_void);
    fn obs_source_restore_filters_free_list(source: *mut c_void);
    fn obs_source_restore_filters_unlock(source: *mut c_void);
    fn obs_source_restore_filters_item_count(array: *mut c_void) -> usize;
    fn obs_source_restore_filters_item(array: *mut c_void, index: usize) -> *mut c_void;
    fn obs_source_restore_filters_item_name(item: *mut c_void) -> *const c_char;
    fn obs_source_restore_filters_name(filter: *const c_void) -> *const c_char;
    fn obs_source_restore_filters_get_ref(filter: *mut c_void) -> *mut c_void;
    fn obs_source_restore_filters_load(item: *mut c_void) -> *mut c_void;
    fn obs_source_restore_filters_load_private(item: *mut c_void) -> *mut c_void;
    fn obs_source_restore_filters_store(
        source: *mut c_void,
        filters: *const *mut c_void,
        count: usize,
    );
    fn obs_source_restore_filters_release_item(item: *mut c_void);
    fn obs_source_restore_filters_release(filter: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// Constructed only for a live, non-NULL source; the core reads `array`
/// only once it has been told it is not NULL.
struct Live {
    source: NonNull<c_void>,
    array: *mut c_void,
}

/// The bytes of a C string, or `None` for NULL.
///
/// # Safety
/// A non-NULL `text` must be NUL-terminated and stay valid and unchanged
/// while the returned slice is used.
unsafe fn bytes<'a>(text: *const c_char) -> Option<&'a [u8]> {
    // SAFETY: the caller's contract.
    (!text.is_null()).then(|| unsafe { CStr::from_ptr(text) }.to_bytes())
}

impl RestoreList for Live {
    type Source = *mut c_void;
    type Item = *mut c_void;
    fn this(&self) -> *mut c_void {
        self.source.as_ptr()
    }
    fn lock(&self) {
        // SAFETY: a live source owns an initialized filter_mutex.
        unsafe { obs_source_restore_filters_lock(self.source.as_ptr()) }
    }
    fn count(&self) -> usize {
        // SAFETY: a live source, read under its filter_mutex.
        unsafe { obs_source_restore_filters_count(self.source.as_ptr()) }
    }
    fn filter(&self, index: usize) -> *mut c_void {
        // SAFETY: the core asks only below a count read under the held
        // filter_mutex, the original C's contract for the array.
        unsafe { obs_source_restore_filters_at(self.source.as_ptr(), index) }
    }
    fn unlink(&self, filter: *mut c_void) {
        // SAFETY: a listed filter, alive (the list's reference, which the
        // core keeps until it releases it), written under the held lock.
        unsafe {
            obs_source_restore_filters_set_parent(filter, null_mut());
            obs_source_restore_filters_set_target(filter, null_mut());
        }
    }
    fn free_list(&self) {
        // SAFETY: a live source's list, under the held lock; the core holds
        // the listed references.
        unsafe { obs_source_restore_filters_free_list(self.source.as_ptr()) }
    }
    fn unlock(&self) {
        // SAFETY: matches the lock the core took on this live source.
        unsafe { obs_source_restore_filters_unlock(self.source.as_ptr()) }
    }
    fn item_count(&self) -> usize {
        // SAFETY: the caller's live, non-NULL array.
        unsafe { obs_source_restore_filters_item_count(self.array) }
    }
    fn item(&self, index: usize) -> *mut c_void {
        // SAFETY: the caller's live, non-NULL array, below its count.
        unsafe { obs_source_restore_filters_item(self.array, index) }
    }
    fn item_name(&self, item: *mut c_void) -> Option<&[u8]> {
        // SAFETY: an item the core holds until after its last use of the
        // name; obs_data_get_string() returns a NUL-terminated string the
        // item owns (never NULL).
        unsafe { bytes(obs_source_restore_filters_item_name(item)) }
    }
    fn name(&self, filter: *mut c_void) -> Option<&[u8]> {
        // SAFETY: an old filter the core still holds; its name is NULL or
        // NUL-terminated and nothing renames it during the compare (the
        // original read it the same way).
        unsafe { bytes(obs_source_restore_filters_name(filter)) }
    }
    fn get_ref(&self, filter: *mut c_void) -> Option<*mut c_void> {
        // SAFETY: an old filter the core still holds.
        let referenced = unsafe { obs_source_restore_filters_get_ref(filter) };
        (!referenced.is_null()).then_some(referenced)
    }
    fn load(&self, item: *mut c_void) -> *mut c_void {
        // SAFETY: an item the core holds.
        unsafe { obs_source_restore_filters_load(item) }
    }
    fn load_private(&self, item: *mut c_void) -> *mut c_void {
        // SAFETY: an item the core holds.
        unsafe { obs_source_restore_filters_load_private(item) }
    }
    fn set_target(&self, filter: *mut c_void, target: *mut c_void) {
        // SAFETY: a restored filter the core holds a reference to (a load
        // reads the new source's fields before it returns, so it is never
        // NULL), as in the original.
        unsafe { obs_source_restore_filters_set_target(filter, target) }
    }
    fn set_parent(&self, filter: *mut c_void) {
        // SAFETY: as for set_target.
        unsafe { obs_source_restore_filters_set_parent(filter, self.source.as_ptr()) }
    }
    fn store(&self, filters: Vec<*mut c_void>) {
        // SAFETY: a live source's list, replaced under the held lock; the
        // adapter copies the filters before the vector is dropped, and the
        // list takes over the references they carry.
        unsafe {
            obs_source_restore_filters_store(self.source.as_ptr(), filters.as_ptr(), filters.len());
        }
    }
    fn release_item(&self, item: *mut c_void) {
        // SAFETY: the item's reference from obs_data_array_item().
        unsafe { obs_source_restore_filters_release_item(item) }
    }
    fn release(&self, filter: *mut c_void) {
        // SAFETY: the old list's reference, released once.
        unsafe { obs_source_restore_filters_release(filter) }
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
/// A non-NULL source must be live with an initialized filter list under
/// the original libobs contract; a non-NULL array must be a live
/// `obs_data_array_t` of `obs_data_t` items.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_restore_filters(source: *mut c_void, array: *mut c_void) {
    // No adapter runs until NULL has been rejected.
    let live = NonNull::new(source).map(|source| Live { source, array });
    restore(live.as_ref(), !array.is_null(), null_log);
}
