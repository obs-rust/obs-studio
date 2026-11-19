//! Converts the private C audio-capture list to the safe ordered-registry
//! interface. The source mutex, darray and bmem storage stay in C.
use super::callback_list::{self, Adapters};
use crate::audio_capture_callbacks;
use core::ffi::c_void;
use obs_util::ffi::darray::darray;

/// `obs_source_audio_capture_t`: (param, source, audio_data, muted).
pub type Callback = Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void, bool)>;
/// `struct audio_cb_info`, with its native pointer alignment.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Pair {
    pub callback: Callback,
    pub param: *mut c_void,
}
impl PartialEq for Pair {
    fn eq(&self, other: &Self) -> bool {
        self.callback.map(|f| f as usize) == other.callback.map(|f| f as usize)
            && self.param == other.param
    }
}
impl Eq for Pair {}
unsafe extern "C" {
    fn obs_source_audio_capture_callback_fields(source: *mut c_void) -> *mut darray;
    fn obs_source_audio_capture_callback_lock(source: *mut c_void);
    fn obs_source_audio_capture_callback_unlock(source: *mut c_void);
}
const ADAPTERS: Adapters = Adapters {
    fields: obs_source_audio_capture_callback_fields,
    lock: obs_source_audio_capture_callback_lock,
    unlock: obs_source_audio_capture_callback_unlock,
};
unsafe fn update(source: *mut c_void, pair: Pair, add: bool) {
    // SAFETY: caller supplies a live source or NULL; NULL takes no adapter.
    let mut locked = unsafe { callback_list::lock::<Pair>(source, &ADAPTERS) };
    audio_capture_callbacks::update(locked.as_mut(), pair, add, callback_list::log);
}
/// # Safety
/// A non-null source must be live; callback and param follow the C API contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_add_audio_capture_callback(
    source: *mut c_void,
    callback: Callback,
    param: *mut c_void,
) {
    // SAFETY: the private entry has the same contract as this API.
    unsafe { obs_source_add_audio_capture_callback_rust(source, callback, param) };
}
/// # Safety
/// A non-null source must be live; callback and param follow the C API contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_remove_audio_capture_callback(
    source: *mut c_void,
    callback: Callback,
    param: *mut c_void,
) {
    // SAFETY: the private entry has the same contract as this API.
    unsafe { obs_source_remove_audio_capture_callback_rust(source, callback, param) };
}
/// Private static-link entry: shared libobs must never satisfy a fixture's
/// Rust registration before its archive member is selected.
///
/// # Safety
/// As obs_source_add_audio_capture_callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_add_audio_capture_callback_rust(
    source: *mut c_void,
    callback: Callback,
    param: *mut c_void,
) {
    // SAFETY: forward the caller's source lifetime contract; NULL is accepted.
    unsafe { update(source, Pair { callback, param }, true) };
}
/// Private static-link entry, as the add entry.
///
/// # Safety
/// As obs_source_remove_audio_capture_callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_remove_audio_capture_callback_rust(
    source: *mut c_void,
    callback: Callback,
    param: *mut c_void,
) {
    // SAFETY: forward the caller's source lifetime contract; NULL is accepted.
    unsafe { update(source, Pair { callback, param }, false) };
}
