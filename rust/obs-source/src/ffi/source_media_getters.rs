//! Thin bridge over real-header adapters; never mirrors the private source layout.
use crate::media_getters::Source;
use core::ffi::{CStr, c_char, c_int, c_void};

type TimeCallback = unsafe extern "C" fn(*mut c_void) -> i64;
type StateCallback = unsafe extern "C" fn(*mut c_void) -> u32;
unsafe extern "C" {
    fn obs_source_media_getter_data(source: *mut c_void) -> *mut c_void;
    fn obs_source_media_getter_flags(source: *mut c_void) -> u32;
    fn obs_source_media_getter_duration_callback(source: *mut c_void) -> Option<TimeCallback>;
    fn obs_source_media_getter_time_callback(source: *mut c_void) -> Option<TimeCallback>;
    fn obs_source_media_getter_state_callback(source: *mut c_void) -> Option<StateCallback>;
    fn blog(level: c_int, format: *const c_char, ...);
}
struct Handle(*mut c_void);
impl Source for Handle {
    fn has_data(&self) -> bool {
        // SAFETY: constructed only for a live non-NULL source for the duration of the query.
        !unsafe { obs_source_media_getter_data(self.0) }.is_null()
    }
    fn output_flags(&self) -> u32 {
        // SAFETY: the handle borrows a live source.
        unsafe { obs_source_media_getter_flags(self.0) }
    }
    fn duration(&self) -> Option<i64> {
        // SAFETY: the callback and current context belong to the borrowed live source.
        unsafe {
            obs_source_media_getter_duration_callback(self.0)
                .map(|cb| cb(obs_source_media_getter_data(self.0)))
        }
    }
    fn time(&self) -> Option<i64> {
        // SAFETY: the callback and current context belong to the borrowed live source.
        unsafe {
            obs_source_media_getter_time_callback(self.0)
                .map(|cb| cb(obs_source_media_getter_data(self.0)))
        }
    }
    fn state(&self) -> Option<u32> {
        // SAFETY: the callback and current context belong to the borrowed live source.
        unsafe {
            obs_source_media_getter_state_callback(self.0)
                .map(|cb| cb(obs_source_media_getter_data(self.0)))
        }
    }
}
fn log(name: &'static CStr) {
    // SAFETY: both strings are static and NUL terminated.
    unsafe { blog(400, c"%s: Null 'source' parameter".as_ptr(), name.as_ptr()) }
}
/// # Safety
/// Non-NULL source and its callback context must remain live through synchronous reentry.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_media_get_duration(source: *mut c_void) -> i64 {
    crate::media_getters::duration((!source.is_null()).then_some(Handle(source)).as_ref(), log)
}
/// # Safety
/// Non-NULL source and its callback context must remain live through synchronous reentry.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_media_get_time(source: *mut c_void) -> i64 {
    crate::media_getters::time((!source.is_null()).then_some(Handle(source)).as_ref(), log)
}
/// # Safety
/// Non-NULL source and its callback context must remain live through synchronous reentry.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_media_get_state(source: *mut c_void) -> u32 {
    crate::media_getters::state((!source.is_null()).then_some(Handle(source)).as_ref(), log)
}
