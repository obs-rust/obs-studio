//! `obs_load_sources()` (#5574) over C-owned state: the data array, the
//! source loads and the source and data references, behind the private
//! adapters of `libobs/obs-load-sources-fields.c.h`. The two-pass walk is the
//! safe core's ([`crate::load_sources`]).
use core::ffi::c_void;
use core::ptr::{self, NonNull};

use crate::load_sources::{Loader, load_sources};

/// `obs_source_t`, opaque here.
pub type Source = c_void;
/// `obs_data_t`, opaque here.
pub type Data = c_void;
/// `obs_data_array_t`, opaque here.
pub type Array = c_void;

/// `obs_load_source_cb`.
pub type LoadCallback = unsafe extern "C" fn(private_data: *mut c_void, source: *mut Source);

unsafe extern "C" {
    fn obs_load_sources_count(array: *mut Array) -> usize;
    fn obs_load_sources_item(array: *mut Array, index: usize) -> *mut Data;
    fn obs_load_sources_release_data(data: *mut Data);
    fn obs_load_sources_load(data: *mut Data) -> *mut Source;
    fn obs_load_sources_is_transition(source: *const Source) -> bool;
    fn obs_load_sources_transition_load(source: *mut Source, data: *mut Data);
    fn obs_load_sources_load2(source: *mut Source);
    fn obs_load_sources_release_source(source: *mut Source);
}

/// The data array and the source calls.
struct CoreLoader;

impl Loader for CoreLoader {
    type Array = *mut Array;
    type Data = *mut Data;
    type Source = NonNull<Source>;
    fn count(&mut self, array: *mut Array) -> usize {
        // SAFETY: the caller's array (or NULL, which counts 0).
        unsafe { obs_load_sources_count(array) }
    }
    fn item(&mut self, array: *mut Array, index: usize) -> *mut Data {
        // SAFETY: as `count`; the new reference is this walk's to release.
        unsafe { obs_load_sources_item(array, index) }
    }
    fn release_data(&mut self, data: *mut Data) {
        // SAFETY: an item this walk took, released once.
        unsafe { obs_load_sources_release_data(data) }
    }
    fn load(&mut self, data: *mut Data) -> Option<NonNull<Source>> {
        // SAFETY: an item this walk holds; the new source reference is this
        // walk's to release.
        NonNull::new(unsafe { obs_load_sources_load(data) })
    }
    fn is_transition(&mut self, source: NonNull<Source>) -> bool {
        // SAFETY: a source this walk holds a reference to.
        unsafe { obs_load_sources_is_transition(source.as_ptr()) }
    }
    fn transition_load(&mut self, source: NonNull<Source>, data: *mut Data) {
        // SAFETY: a held source and the item this walk holds for its entry.
        unsafe { obs_load_sources_transition_load(source.as_ptr(), data) }
    }
    fn load2(&mut self, source: NonNull<Source>) {
        // SAFETY: as `is_transition`.
        unsafe { obs_load_sources_load2(source.as_ptr()) }
    }
    fn release_source(&mut self, source: Option<NonNull<Source>>) {
        // SAFETY: a reference this walk took (or NULL, a no-op), released
        // once.
        unsafe { obs_load_sources_release_source(source.map_or(ptr::null_mut(), NonNull::as_ptr)) }
    }
}

/// `obs_load_sources(array, cb, private_data)`: every entry of `array`
/// loaded, then each loaded source given its transition load, second-pass
/// load and callback, in array order, then every reference dropped.
///
/// # Safety
/// As for the C function: the core is initialized, `array` is a live data
/// array or NULL, and `cb`, when not NULL, may be called with `private_data`
/// and any loaded source.
pub unsafe fn load_sources_bridge(
    array: *mut Array,
    cb: Option<LoadCallback>,
    private_data: *mut c_void,
) {
    let cb = cb.map(|cb| {
        move |source: NonNull<Source>| {
            // SAFETY: the caller's callback with its own private data and a
            // source this walk holds, as the original calls it.
            unsafe { cb(private_data, source.as_ptr()) }
        }
    });
    load_sources(&mut CoreLoader, array, cb);
}
