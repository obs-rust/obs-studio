//! Thin bridge: C owns the source and context layouts, the name tables, the
//! canvas rename and signal delivery; Rust orders the rename steps.
use core::ffi::{c_char, c_int, c_void};
use core::ptr;
use std::ffi::CStr;

use obs_callback::ffi::calldata::{calldata, calldata_set_data};
use obs_util::ffi::bmem::{bfree, bmemdup};

use crate::set_name::{self, Source};

unsafe extern "C" {
    fn obs_source_set_name_requires_canvas(source: *const c_void) -> bool;
    fn obs_source_set_name_current(source: *const c_void) -> *const c_char;
    fn obs_source_set_name_private(source: *const c_void) -> bool;
    fn obs_source_set_name_canvas(source: *mut c_void, name: *const c_char);
    fn obs_source_set_name_public(source: *mut c_void, name: *const c_char);
    fn obs_source_set_name_unlisted(source: *mut c_void, name: *const c_char);
    fn obs_source_set_name_global(source: *mut c_void, data: *mut calldata);
    fn obs_source_set_name_local(source: *mut c_void, data: *mut calldata);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// `LOG_DEBUG` in `util/base.h`.
const LOG_DEBUG: c_int = 400;

struct Live {
    source: *mut c_void,
    name: *const c_char,
}

/// The bytes of a C string, or `None` for NULL.
///
/// # Safety
/// A non-null `text` is a live NUL-terminated string for `'a`.
unsafe fn bytes<'a>(text: *const c_char) -> Option<&'a [u8]> {
    // SAFETY: the caller's contract.
    (!text.is_null()).then(|| unsafe { CStr::from_ptr(text) }.to_bytes())
}

/// `calldata_set_string()`: the string with its terminator, or no bytes for NULL.
///
/// # Safety
/// `data` is a live calldata and a non-null `text` a live C string.
unsafe fn set_string(data: &mut calldata, key: &CStr, text: *const c_char) {
    // SAFETY: the caller's contract; calldata copies the bytes.
    unsafe {
        match bytes(text) {
            Some(text) => {
                calldata_set_data(data, key.as_ptr(), text.as_ptr().cast(), text.len() + 1)
            }
            None => calldata_set_data(data, key.as_ptr(), ptr::null(), 0),
        }
    }
}

impl Source for Live {
    type Previous = *mut c_char;
    type Data = calldata;

    fn requested(&self) -> Option<&[u8]> {
        // SAFETY: the ABI caller keeps the requested name live for the call.
        unsafe { bytes(self.name) }
    }
    fn current(&self) -> Option<&[u8]> {
        // SAFETY: the source is live; the borrow ends before any C call
        // that could rename it.
        unsafe { bytes(obs_source_set_name_current(self.source)) }
    }
    fn requires_canvas(&self) -> bool {
        // SAFETY: the ABI caller keeps the source live.
        unsafe { obs_source_set_name_requires_canvas(self.source) }
    }
    fn private(&self) -> bool {
        // SAFETY: the ABI caller keeps the source live.
        unsafe { obs_source_set_name_private(self.source) }
    }
    fn rename_canvas(&mut self) {
        // SAFETY: both pointers stay live; this is the original canvas call.
        unsafe { obs_source_set_name_canvas(self.source, self.name) }
    }
    fn save_previous(&mut self) -> *mut c_char {
        // SAFETY: the context name is NULL or a live C string; bstrdup's copy
        // with libobs allocation, NULL for NULL.
        unsafe {
            match bytes(obs_source_set_name_current(self.source)) {
                Some(name) => bmemdup(name.as_ptr().cast(), name.len() + 1).cast(),
                None => ptr::null_mut(),
            }
        }
    }
    fn rename_public(&mut self) {
        // SAFETY: both pointers stay live; C passes the public source table.
        unsafe { obs_source_set_name_public(self.source, self.name) }
    }
    fn rename_private(&mut self) {
        // SAFETY: both pointers stay live; this is the plain context setter.
        unsafe { obs_source_set_name_unlisted(self.source, self.name) }
    }
    fn data(&mut self, previous: &*mut c_char) -> calldata {
        let mut data = calldata {
            stack: ptr::null_mut(),
            size: 0,
            capacity: 0,
            fixed: false,
        };
        // SAFETY: the source is live, the context name is read after the
        // rename as in C, and the previous name is this call's own copy.
        unsafe {
            calldata_set_data(
                &mut data,
                c"source".as_ptr(),
                (&raw const self.source).cast(),
                size_of::<*mut c_void>(),
            );
            set_string(
                &mut data,
                c"new_name",
                obs_source_set_name_current(self.source),
            );
            set_string(&mut data, c"prev_name", *previous);
        }
        data
    }
    fn signal_global(&mut self, data: &mut calldata) {
        // SAFETY: the payload lives through the synchronous callbacks.
        unsafe { obs_source_set_name_global(self.source, data) }
    }
    fn signal_local(&mut self, data: &mut calldata) {
        // SAFETY: the source and payload live through the synchronous callbacks.
        unsafe { obs_source_set_name_local(self.source, data) }
    }
    fn free_data(&mut self, data: calldata) {
        if !data.fixed {
            // SAFETY: as in calldata_free, release only a non-fixed libobs stack.
            unsafe { bfree(data.stack.cast()) }
        }
    }
    fn free_previous(&mut self, previous: *mut c_char) {
        // SAFETY: save_previous allocated this copy (or NULL) with libobs allocation.
        unsafe { bfree(previous.cast()) }
    }
}

/// # Safety
/// A non-null source must stay live for the call, including its synchronous
/// callbacks, and a non-null name must be a live C string, under the original
/// setter's ownership and synchronization rules.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_set_name(source: *mut c_void, name: *const c_char) {
    let mut live = (!source.is_null()).then_some(Live { source, name });
    set_name::set_name(live.as_mut(), || {
        // SAFETY: literal format and NUL-terminated arguments match %s %s.
        unsafe {
            blog(
                LOG_DEBUG,
                c"%s: Null '%s' parameter".as_ptr(),
                c"obs_source_set_name".as_ptr(),
                c"source".as_ptr(),
            )
        };
    });
}
