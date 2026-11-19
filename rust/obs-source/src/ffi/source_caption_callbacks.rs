//! Converts the private C list to the safe ordered-registry interface.
use super::callback_list::{self, Adapters};
use crate::caption_callbacks;
use core::ffi::c_void;
use obs_util::ffi::darray::darray;

pub type Callback = Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void)>;
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
    fn obs_source_caption_callback_fields(source: *mut c_void) -> *mut darray;
    fn obs_source_caption_callback_lock(source: *mut c_void);
    fn obs_source_caption_callback_unlock(source: *mut c_void);
}
const ADAPTERS: Adapters = Adapters {
    fields: obs_source_caption_callback_fields,
    lock: obs_source_caption_callback_lock,
    unlock: obs_source_caption_callback_unlock,
};
unsafe fn update(source: *mut c_void, pair: Pair, add: bool) {
    // SAFETY: caller supplies a live source or NULL; NULL takes no adapter.
    let mut locked = unsafe { callback_list::lock::<Pair>(source, &ADAPTERS) };
    caption_callbacks::update(locked.as_mut(), pair, add, callback_list::log);
}
/// # Safety
/// A non-null source must be live; callback and param follow the C API contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_add_caption_callback(
    source: *mut c_void,
    callback: Callback,
    param: *mut c_void,
) {
    // SAFETY: forward the caller's source lifetime contract; NULL is accepted.
    unsafe { update(source, Pair { callback, param }, true) };
}
/// # Safety
/// A non-null source must be live; callback and param follow the C API contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_remove_caption_callback(
    source: *mut c_void,
    callback: Callback,
    param: *mut c_void,
) {
    // SAFETY: forward the caller's source lifetime contract; NULL is accepted.
    unsafe { update(source, Pair { callback, param }, false) };
}
