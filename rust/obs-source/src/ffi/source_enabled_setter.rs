//! Private field access and calldata conversion for the enabled setter.
use crate::enabled_setter::{self, Source};
use core::ffi::{c_char, c_int, c_void};
use obs_callback::ffi::{
    calldata::{calldata, calldata_set_data},
    signal::{signal_handler, signal_handler_signal},
};

unsafe extern "C" {
    fn obs_source_enabled_setter_field(source: *mut c_void) -> *mut bool;
    fn obs_source_enabled_setter_signals(source: *mut c_void) -> *mut signal_handler;
    fn blog(level: c_int, format: *const c_char, ...);
}

struct BorrowedSource(*mut c_void);

impl Source for BorrowedSource {
    fn store_enabled(&self, enabled: bool) {
        // SAFETY: constructed only for a live non-null source. This plain write
        // has the original C synchronization contract. No reference escapes.
        unsafe { obs_source_enabled_setter_field(self.0).write(enabled) };
    }

    fn signal_enable(&self, enabled: bool) {
        let mut stack = [0_u8; 128];
        let mut data = calldata {
            stack: stack.as_mut_ptr(),
            size: size_of::<usize>(),
            capacity: stack.len(),
            fixed: true,
        };
        let value = enabled;
        // SAFETY: live source and handler; names are static C strings, values
        // have their C sizes, and the fixed stack stays live through dispatch.
        // No reference to the source's fields spans a reentrant callback.
        unsafe {
            calldata_set_data(
                &mut data,
                c"source".as_ptr(),
                (&raw const self.0).cast(),
                size_of::<*mut c_void>(),
            );
            calldata_set_data(
                &mut data,
                c"enabled".as_ptr(),
                (&raw const value).cast(),
                size_of::<bool>(),
            );
            signal_handler_signal(
                obs_source_enabled_setter_signals(self.0),
                c"enable".as_ptr(),
                &mut data,
            );
        }
    }
}

/// # Safety
/// A non-null source must remain live for the call, including its synchronous
/// callbacks, under the original C setter's ownership and synchronization rules.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_set_enabled(source: *mut c_void, enabled: bool) {
    let source = (!source.is_null()).then_some(BorrowedSource(source));
    enabled_setter::set_enabled(source.as_ref(), enabled, || {
        // SAFETY: literal format and NUL-terminated argument match %s.
        unsafe {
            blog(
                400,
                c"%s: Null 'source' parameter".as_ptr(),
                c"obs_source_set_enabled".as_ptr(),
            )
        };
    });
}
