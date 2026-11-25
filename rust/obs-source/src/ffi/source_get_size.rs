//! C ABI of `obs_source_get_width()` and `obs_source_get_height()`. The
//! private fields and `filter_mutex` stay C, reached through
//! `obs-source-get-size-fields.c.h` (no `EXPORT`); the base size is
//! `obs_source_base_width()` / `obs_source_base_height()`.
use crate::get_size::{self, Axis};
use core::ffi::{CStr, c_char, c_int, c_void};
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_source_get_size_has_data(source: *const c_void) -> bool;
    fn obs_source_get_size_type(source: *const c_void) -> c_int;
    fn obs_source_get_size_lock(source: *mut c_void);
    fn obs_source_get_size_unlock(source: *mut c_void);
    fn obs_source_get_size_has_filters(source: *const c_void) -> bool;
    fn obs_source_get_size_first_filter(source: *const c_void) -> *mut c_void;
    fn obs_source_base_width(source: *const c_void) -> u32;
    fn obs_source_base_height(source: *const c_void) -> u32;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// `LOG_DEBUG`, the level of `obs_ptr_valid()`'s NULL line.
const LOG_DEBUG: c_int = 400;

/// What the shims reach: libobs's C. Each source is non-NULL and, under the
/// libobs contract, live for the call.
#[derive(Debug, Default, Clone, Copy)]
pub struct Libobs;

impl get_size::Libobs for Libobs {
    type Source = NonNull<c_void>;

    fn log_null(&mut self, function: &'static CStr) {
        // SAFETY: static strings match the two %s arguments of the format.
        unsafe {
            blog(
                LOG_DEBUG,
                c"%s: Null '%s' parameter".as_ptr(),
                function.as_ptr(),
                get_size::SOURCE.as_ptr(),
            );
        }
    }

    fn has_data(&mut self, source: Self::Source) -> bool {
        // SAFETY: a live source; the adapter reads one field.
        unsafe { obs_source_get_size_has_data(source.as_ptr()) }
    }

    fn source_type(&mut self, source: Self::Source) -> i32 {
        // SAFETY: as `has_data`.
        unsafe { obs_source_get_size_type(source.as_ptr()) }
    }

    fn lock_filters(&mut self, source: Self::Source) {
        // SAFETY: a live source with an initialized filter_mutex.
        unsafe { obs_source_get_size_lock(source.as_ptr()) }
    }

    fn unlock_filters(&mut self, source: Self::Source) {
        // SAFETY: the core unlocks only what it locked, on this thread.
        unsafe { obs_source_get_size_unlock(source.as_ptr()) }
    }

    fn has_filters(&mut self, source: Self::Source) -> bool {
        // SAFETY: a live source whose filter_mutex this thread holds.
        unsafe { obs_source_get_size_has_filters(source.as_ptr()) }
    }

    fn first_filter(&mut self, source: Self::Source) -> Option<Self::Source> {
        // SAFETY: as `has_filters`, and the list is not empty.
        NonNull::new(unsafe { obs_source_get_size_first_filter(source.as_ptr()) })
    }

    fn base_size(&mut self, source: Self::Source, axis: Axis) -> u32 {
        // SAFETY: a live source (the source or its first filter, under its
        // parent's filter_mutex).
        unsafe {
            match axis {
                Axis::Width => obs_source_base_width(source.as_ptr()),
                Axis::Height => obs_source_base_height(source.as_ptr()),
            }
        }
    }
}

/// `obs_source_get_width()` / `_height()` over any [`get_size::Libobs`]
/// whose sources are pointers (the exports use [`Libobs`]; the parity test
/// passes mutants of it).
pub fn get_with<L: get_size::Libobs<Source = NonNull<c_void>>>(
    libobs: &mut L,
    source: *mut c_void,
    axis: Axis,
) -> u32 {
    get_size::get(libobs, NonNull::new(source), axis)
}

/// # Safety
/// A non-null source must be live under the original libobs ownership
/// contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_width(source: *mut c_void) -> u32 {
    get_with(&mut Libobs, source, Axis::Width)
}

/// # Safety
/// A non-null source must be live under the original libobs ownership
/// contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_height(source: *mut c_void) -> u32 {
    get_with(&mut Libobs, source, Axis::Height)
}
