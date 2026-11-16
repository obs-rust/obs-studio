//! Thin bridge: C owns canvas/context layouts; Rust orders the rename operations.
use core::ffi::{c_char, c_void};
use core::ptr;
use std::ffi::CStr;

use obs_callback::ffi::calldata::{calldata, calldata_set_data};
use obs_util::ffi::bmem::{bfree, bmemdup};

use crate::canvas_set_name::{self, Host};

unsafe extern "C" {
    fn obs_canvas_rename_flags(canvas: *const c_void) -> u32;
    fn obs_canvas_rename_name(canvas: *const c_void) -> *const c_char;
    fn obs_canvas_rename_private(canvas: *const c_void) -> bool;
    fn obs_canvas_rename_set_private(canvas: *mut c_void, name: *const c_char);
    fn obs_canvas_rename_set_public(canvas: *mut c_void, name: *const c_char);
    fn obs_canvas_rename_local(canvas: *mut c_void, data: *mut calldata);
    fn obs_canvas_rename_global(canvas: *mut c_void, data: *mut calldata);
}

struct Canvas {
    canvas: *mut c_void,
    name: *const c_char,
}

impl Host for Canvas {
    type Previous = *mut c_char;
    type Data = calldata;

    fn requested(&self) -> Option<&[u8]> {
        if self.name.is_null() {
            None
        } else {
            // SAFETY: the ABI caller keeps the requested C string live for the call.
            Some(unsafe { CStr::from_ptr(self.name) }.to_bytes())
        }
    }
    fn flags(&self) -> u32 {
        // SAFETY: the ABI caller retains a live canvas.
        unsafe { obs_canvas_rename_flags(self.canvas) }
    }
    fn current(&self) -> &[u8] {
        // SAFETY: the original strcmp requires a live, non-null context name.
        unsafe { CStr::from_ptr(obs_canvas_rename_name(self.canvas)) }.to_bytes()
    }
    fn private(&self) -> bool {
        // SAFETY: the ABI caller retains a live canvas, also across callbacks.
        unsafe { obs_canvas_rename_private(self.canvas) }
    }
    fn save_previous(&mut self) -> Self::Previous {
        // SAFETY: current() has the original strcmp's valid-name precondition.
        let name = unsafe { CStr::from_ptr(obs_canvas_rename_name(self.canvas)) };
        let bytes = name.to_bytes_with_nul();
        // SAFETY: copy exactly the live string including its terminator using libobs allocation.
        unsafe { bmemdup(bytes.as_ptr().cast(), bytes.len()).cast() }
    }
    fn rename_private(&mut self) {
        // SAFETY: both pointers remain live; this is the original context setter.
        unsafe { obs_canvas_rename_set_private(self.canvas, self.name) }
    }
    fn rename_public(&mut self) {
        // SAFETY: both pointers remain live; C supplies the real name hash table.
        unsafe { obs_canvas_rename_set_public(self.canvas, self.name) }
    }
    fn data(&mut self, previous: &Self::Previous) -> calldata {
        let mut data = calldata {
            stack: ptr::null_mut(),
            size: 0,
            capacity: 0,
            fixed: false,
        };
        // SAFETY: these are the original live canvas and owned previous-name strings.
        unsafe {
            calldata_set_data(
                &mut data,
                c"canvas".as_ptr(),
                (&raw const self.canvas).cast(),
                size_of::<*mut c_void>(),
            );
            let name = CStr::from_ptr(obs_canvas_rename_name(self.canvas));
            calldata_set_data(
                &mut data,
                c"new_name".as_ptr(),
                name.as_ptr().cast(),
                name.to_bytes_with_nul().len(),
            );
            let previous = CStr::from_ptr(*previous);
            calldata_set_data(
                &mut data,
                c"prev_name".as_ptr(),
                previous.as_ptr().cast(),
                previous.to_bytes_with_nul().len(),
            );
        }
        data
    }
    fn signal_local(&mut self, data: &mut calldata) {
        // SAFETY: the canvas and mutable payload live through synchronous callbacks.
        unsafe { obs_canvas_rename_local(self.canvas, data) }
    }
    fn signal_global(&mut self, data: &mut calldata) {
        // SAFETY: the core signal handler and payload live through synchronous callbacks.
        unsafe { obs_canvas_rename_global(self.canvas, data) }
    }
    fn free_data(&mut self, data: calldata) {
        if !data.fixed {
            // SAFETY: as in calldata_free, release only a non-fixed libobs stack.
            unsafe { bfree(data.stack.cast()) }
        }
    }
    fn free_previous(&mut self, previous: Self::Previous) {
        // SAFETY: save_previous allocated this unique copy with libobs allocation.
        unsafe { bfree(previous.cast()) }
    }
}

/// # Safety
/// A nonempty name requires a live canvas; a non-main canvas must have a live,
/// non-null current name, as for the original C strcmp. The caller retains the
/// canvas through callbacks and synchronizes renames under the original API rules.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_canvas_set_name(canvas: *mut c_void, name: *const c_char) {
    canvas_set_name::set_name(&mut Canvas { canvas, name });
}
