//! Translate opaque handles to the safe backup sequence.
use crate::filter_backup::{self, Backend};
use core::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_source_filter_backup_lock(source: *mut c_void);
    fn obs_source_filter_backup_unlock(source: *mut c_void);
    fn obs_source_filter_backup_count(source: *const c_void) -> usize;
    fn obs_source_filter_backup_at(source: *mut c_void, index: usize) -> *mut c_void;
    fn obs_save_source(source: *mut c_void) -> *mut c_void;
    fn obs_data_array_create() -> *mut c_void;
    fn obs_data_array_push_back(array: *mut c_void, data: *mut c_void) -> usize;
    fn obs_data_release(data: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

// Constructed only after the public shim rejects NULL. The C caller guarantees
// the source's lifetime and initialized recursive mutex for the entire call.
struct Source(*mut c_void);

impl Backend for Source {
    type Array = *mut c_void;
    type Saved = *mut c_void;

    fn create_array(&mut self) -> Self::Array {
        // SAFETY: the libobs allocator returns an owned array.
        unsafe { obs_data_array_create() }
    }
    fn lock(&mut self) {
        // SAFETY: Source contains a live non-NULL handle with an initialized mutex.
        unsafe { obs_source_filter_backup_lock(self.0) }
    }
    fn count(&self) -> usize {
        // SAFETY: the core reads the live list only while its mutex is held.
        unsafe { obs_source_filter_backup_count(self.0) }
    }
    fn save(&mut self, index: usize) -> Self::Saved {
        // SAFETY: the core just checked index against the current locked list.
        // No Rust slice or reference spans the reentrant save callback.
        unsafe { obs_save_source(obs_source_filter_backup_at(self.0, index)) }
    }
    fn push(&mut self, array: &mut Self::Array, saved: &Self::Saved) {
        // SAFETY: both handles came from the libobs create/save APIs and are live.
        unsafe { obs_data_array_push_back(*array, *saved) };
    }
    fn release(&mut self, saved: Self::Saved) {
        // SAFETY: save returned an owned reference; push retained its own reference.
        unsafe { obs_data_release(saved) }
    }
    fn unlock(&mut self) {
        // SAFETY: matches the core's earlier lock of this same live source.
        unsafe { obs_source_filter_backup_unlock(self.0) }
    }
}

fn log(_: &'static str) {
    // SAFETY: the diagnostic is a static NUL-terminated literal with no arguments.
    unsafe {
        blog(
            400,
            c"obs_source_backup_filters: Null 'source' parameter".as_ptr(),
        )
    }
}

/// # Safety
/// A non-NULL source must remain live with its initialized filter mutex and list
/// under the original libobs contract. The result owns an array reference.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_backup_filters(source: *mut c_void) -> *mut c_void {
    // Reject NULL before any C adapter computes a field address.
    let mut source = (!source.is_null()).then_some(Source(source));
    filter_backup::backup(source.as_mut(), log).unwrap_or(core::ptr::null_mut())
}
