//! The source's fields and signal handler stay behind the adapters of
//! `obs-source-flags-mixers-fields.c.h`. Both signals go through libobs's
//! signal handler on a fixed 128-byte calldata, as the C's
//! `calldata_init_fixed()` did.
use crate::flags_mixers::{self, Source};
use core::ffi::{CStr, c_char, c_int, c_void};
use obs_callback::ffi::{
    calldata::{calldata, calldata_get_data, calldata_set_data},
    signal::{signal_handler, signal_handler_signal},
};

unsafe extern "C" {
    fn obs_source_flags_mixers_flags(source: *const c_void) -> u32;
    fn obs_source_flags_mixers_store_flags(source: *mut c_void, flags: u32);
    fn obs_source_flags_mixers_owns_info_id(source: *const c_void) -> bool;
    fn obs_source_flags_mixers_output_flags(source: *const c_void) -> u32;
    fn obs_source_flags_mixers_audio_mixers(source: *const c_void) -> u32;
    fn obs_source_flags_mixers_store_audio_mixers(source: *mut c_void, mixers: u32);
    fn obs_source_flags_mixers_signals(source: *const c_void) -> *mut signal_handler;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// A live, non-NULL source, read, written and signalled through the
/// adapters (public for the parity test's mutants).
pub struct Live(*mut c_void);

impl Live {
    /// # Safety
    /// `source` is a live, non-NULL source that stays live while this value
    /// is used, under the original C setters' contract.
    #[must_use]
    pub unsafe fn new(source: *mut c_void) -> Self {
        Self(source)
    }

    /// Emits `signal` with `source` and the `int` parameter `name` set to
    /// `value`, and answers what the handlers left in `name`
    /// (`calldata_int()`: 0 when it no longer has an `int`'s size).
    fn emit(&self, signal: &CStr, name: &CStr, value: i64) -> i64 {
        // calldata_init_fixed(): an empty fixed stack starts with its size_t
        // terminator, zeroed.
        let mut stack = [0_u8; 128];
        let mut data = calldata {
            stack: stack.as_mut_ptr(),
            size: size_of::<usize>(),
            capacity: stack.len(),
            fixed: true,
        };
        let mut left: i64 = 0;
        // SAFETY: a live source and its handler; the names are static C
        // strings, the values have their C sizes, and the fixed stack stays
        // live through the synchronous dispatch. No reference to the
        // source's fields spans a handler, which may call back in.
        unsafe {
            calldata_set_data(
                &mut data,
                c"source".as_ptr(),
                (&raw const self.0).cast(),
                size_of::<*mut c_void>(),
            );
            calldata_set_data(
                &mut data,
                name.as_ptr(),
                (&raw const value).cast(),
                size_of::<i64>(),
            );
            signal_handler_signal(
                obs_source_flags_mixers_signals(self.0),
                signal.as_ptr(),
                &mut data,
            );
            calldata_get_data(
                &data,
                name.as_ptr(),
                (&raw mut left).cast(),
                size_of::<i64>(),
            );
        }
        left
    }
}

impl Source for Live {
    fn flags(&self) -> u32 {
        // SAFETY: a live source, read under the C setter's contract.
        unsafe { obs_source_flags_mixers_flags(self.0) }
    }
    fn store_flags(&self, flags: u32) {
        // SAFETY: a live source; a plain write, as the C did.
        unsafe { obs_source_flags_mixers_store_flags(self.0, flags) }
    }
    fn signal_update_flags(&self) {
        let flags = self.flags();
        self.emit(c"update_flags", c"flags", i64::from(flags));
    }
    fn owns_info_id(&self) -> bool {
        // SAFETY: a live source.
        unsafe { obs_source_flags_mixers_owns_info_id(self.0) }
    }
    fn output_flags(&self) -> u32 {
        // SAFETY: a live source.
        unsafe { obs_source_flags_mixers_output_flags(self.0) }
    }
    fn audio_mixers(&self) -> u32 {
        // SAFETY: a live source.
        unsafe { obs_source_flags_mixers_audio_mixers(self.0) }
    }
    fn signal_audio_mixers(&self, mixers: i64) -> i64 {
        self.emit(c"audio_mixers", c"mixers", mixers)
    }
    fn store_audio_mixers(&self, mixers: u32) {
        // SAFETY: a live source; a plain write, as the C did.
        unsafe { obs_source_flags_mixers_store_audio_mixers(self.0, mixers) }
    }
}

fn null_log(function: &'static CStr) {
    // SAFETY: the literal format and the static name match its one %s.
    unsafe {
        blog(
            400,
            c"%s: Null 'source' parameter".as_ptr(),
            function.as_ptr(),
        );
    }
}

/// # Safety
/// A non-NULL source must stay live through the call, including its
/// synchronous `update_flags` handlers, under the original C setter's
/// ownership and synchronization rules.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_set_flags(source: *mut c_void, flags: u32) {
    // No adapter runs until NULL has been rejected.
    let source = (!source.is_null()).then_some(Live(source));
    flags_mixers::set_flags(source.as_ref(), flags, null_log);
}

/// # Safety
/// A non-NULL source must stay live through the call, including its
/// synchronous `audio_mixers` handlers, under the original C setter's
/// ownership and synchronization rules.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_set_audio_mixers(source: *mut c_void, mixers: u32) {
    // No adapter runs until NULL has been rejected.
    let source = (!source.is_null()).then_some(Live(source));
    flags_mixers::set_audio_mixers(source.as_ref(), mixers, null_log);
}
