//! `obs_save_sources_filtered()` and `obs_save_sources()` (#5567) over
//! C-owned state: the core's uuid table, its mutex and links, the four
//! fields the save checks, and the array, save, push and release calls,
//! behind the private adapters of `libobs/obs-save-sources-fields.c.h`. The
//! walk and the selection are the safe core's ([`crate::save_sources`]).
use core::ffi::c_void;
use core::ptr::NonNull;

use crate::save_sources::{Sources, save_sources, save_sources_filtered};

/// `obs_source_t`, opaque here.
pub type Source = c_void;
/// `obs_data_t`, opaque here.
pub type Data = c_void;
/// `obs_data_array_t`, opaque here.
pub type Array = c_void;

/// `obs_save_source_filter_cb`.
pub type Filter = unsafe extern "C" fn(param: *mut c_void, source: *mut Source) -> bool;

unsafe extern "C" {
    fn obs_save_sources_lock();
    fn obs_save_sources_unlock();
    fn obs_save_sources_first() -> *mut Source;
    fn obs_save_sources_next(source: *const Source) -> *mut Source;
    fn obs_save_sources_is_filter(source: *const Source) -> bool;
    fn obs_save_sources_removed(source: *const Source) -> bool;
    fn obs_save_sources_temp_removed(source: *const Source) -> bool;
    fn obs_save_sources_private(source: *const Source) -> bool;
    fn obs_save_sources_array_create() -> *mut Array;
    fn obs_save_sources_save(source: *mut Source) -> *mut Data;
    fn obs_save_sources_push(array: *mut Array, data: *mut Data);
    fn obs_save_sources_release(data: *mut Data);
}

/// The core's uuid table and the data calls.
struct CoreSources;

impl Sources for CoreSources {
    type Source = NonNull<Source>;
    type Data = *mut Data;
    type Array = *mut Array;
    fn array_create(&mut self) -> *mut Array {
        // SAFETY: no precondition; the caller owns the new array.
        unsafe { obs_save_sources_array_create() }
    }
    fn lock(&mut self) {
        // SAFETY: the initialized core's sources mutex.
        unsafe { obs_save_sources_lock() }
    }
    fn unlock(&mut self) {
        // SAFETY: paired with this walk's own `lock`.
        unsafe { obs_save_sources_unlock() }
    }
    fn first(&mut self) -> Option<NonNull<Source>> {
        // SAFETY: the table head, read under the sources mutex.
        NonNull::new(unsafe { obs_save_sources_first() })
    }
    fn next(&mut self, source: NonNull<Source>) -> Option<NonNull<Source>> {
        // SAFETY: a listed source, live while the sources mutex is held.
        NonNull::new(unsafe { obs_save_sources_next(source.as_ptr()) })
    }
    fn is_filter(&mut self, source: NonNull<Source>) -> bool {
        // SAFETY: as `next`.
        unsafe { obs_save_sources_is_filter(source.as_ptr()) }
    }
    fn removed(&mut self, source: NonNull<Source>) -> bool {
        // SAFETY: as `next`.
        unsafe { obs_save_sources_removed(source.as_ptr()) }
    }
    fn temp_removed(&mut self, source: NonNull<Source>) -> bool {
        // SAFETY: as `next`.
        unsafe { obs_save_sources_temp_removed(source.as_ptr()) }
    }
    fn private(&mut self, source: NonNull<Source>) -> bool {
        // SAFETY: as `next`.
        unsafe { obs_save_sources_private(source.as_ptr()) }
    }
    fn save(&mut self, source: NonNull<Source>) -> *mut Data {
        // SAFETY: a listed source, saved under the sources mutex as the
        // original does; the new data is this walk's to release.
        unsafe { obs_save_sources_save(source.as_ptr()) }
    }
    fn push(&mut self, array: *mut Array, data: *mut Data) {
        // SAFETY: this walk's own array and the data it just saved.
        unsafe { obs_save_sources_push(array, data) }
    }
    fn release(&mut self, data: *mut Data) {
        // SAFETY: the data this walk saved, released once after its push.
        unsafe { obs_save_sources_release(data) }
    }
}

/// `obs_save_sources_filtered(cb, param)`: a new array of the saves of every
/// public, listed, non-filter source `cb` keeps, in table order. A NULL `cb`
/// keeps none, where the C calls NULL.
///
/// # Safety
/// As for the C function: the core is initialized and `cb`, when not NULL,
/// may be called with `param` and any listed source.
pub unsafe fn save_sources_filtered_bridge(cb: Option<Filter>, param: *mut c_void) -> *mut Array {
    save_sources_filtered(&mut CoreSources, |source: NonNull<Source>| {
        // SAFETY: the caller's callback with its own parameter and a listed
        // source, under the sources mutex, as the original calls it.
        cb.is_some_and(|cb| unsafe { cb(param, source.as_ptr()) })
    })
}

/// `obs_save_sources()`: [`save_sources_filtered_bridge`] keeping every
/// candidate.
///
/// # Safety
/// As for the C function: the core is initialized.
pub unsafe fn save_sources_bridge() -> *mut Array {
    save_sources(&mut CoreSources)
}
