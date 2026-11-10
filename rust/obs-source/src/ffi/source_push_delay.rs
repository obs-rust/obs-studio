//! C ABI delay setters over the existing query fields and signal adapter.
use super::source_push_queries::{Source, null_log};
use crate::push_delay::{self, DelayFields};
use core::ffi::{CStr, c_void};
use obs_callback::ffi::{
    calldata::{calldata, calldata_set_data},
    signal::{signal_handler, signal_handler_signal},
};
unsafe extern "C" {
    fn obs_source_enabled_setter_signals(source: *mut c_void) -> *mut signal_handler;
}
impl DelayFields for Source {
    fn store_delay(&mut self, talk: bool, delay: u64) {
        let field = if talk {
            self.0.talk_delay
        } else {
            self.0.mute_delay
        };
        // SAFETY: points into a live mutable source; core holds its audio mutex.
        unsafe { field.cast_mut().write(delay) };
    }
    fn signal_delay(&mut self, signal: &'static CStr, delay: i64) {
        let mut stack = [0_u8; 128];
        let mut data = calldata {
            stack: stack.as_mut_ptr(),
            size: size_of::<usize>(),
            capacity: stack.len(),
            fixed: true,
        };
        // SAFETY: source is live, payload has the exact C widths, and fixed
        // storage remains live through synchronous callbacks. No field reference
        // spans dispatch; callbacks can reenter on independent sources.
        unsafe {
            calldata_set_data(
                &mut data,
                c"source".as_ptr(),
                (&raw const self.1).cast(),
                size_of::<*mut c_void>(),
            );
            calldata_set_data(
                &mut data,
                c"delay".as_ptr(),
                (&raw const delay).cast(),
                size_of::<i64>(),
            );
            signal_handler_signal(
                obs_source_enabled_setter_signals(self.1),
                signal.as_ptr(),
                &mut data,
            );
        }
    }
}
/// # Safety
/// A non-NULL source must remain live through synchronous callbacks and have
/// an initialized audio mutex, following the original C ownership contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_set_push_to_mute_delay(source: *mut c_void, delay: u64) {
    // SAFETY: public contract supplies a live source or NULL; borrow checks NULL.
    let mut source = unsafe { Source::borrow(source) };
    push_delay::set_delay(source.as_mut(), false, delay, null_log);
}
/// # Safety
/// A non-NULL source must remain live through synchronous callbacks and have
/// an initialized audio mutex, following the original C ownership contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_set_push_to_talk_delay(source: *mut c_void, delay: u64) {
    // SAFETY: public contract supplies a live source or NULL; borrow checks NULL.
    let mut source = unsafe { Source::borrow(source) };
    push_delay::set_delay(source.as_mut(), true, delay, null_log);
}
