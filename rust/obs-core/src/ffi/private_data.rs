//! Thin private-data bridge; the core and obs-data objects retain C ownership.
use crate::private_data::{self, Host};
use std::ffi::c_void;
use std::ptr::NonNull;

unsafe extern "C" {
    fn obs_core_private_data_field() -> *const *mut c_void;
    fn obs_data_clear(data: *mut c_void);
    fn obs_data_apply(data: *mut c_void, settings: *mut c_void);
    fn obs_data_addref(data: *mut c_void);
}

struct Calls;
impl Host for Calls {
    type Data = *mut c_void;
    fn private_data(&mut self) -> Self::Data {
        // SAFETY: both exports require the initialized core and its live field.
        unsafe { *obs_core_private_data_field() }
    }
    fn clear(&mut self, data: Self::Data) {
        // SAFETY: the core owns this live data object throughout the call.
        unsafe { obs_data_clear(data) }
    }
    fn apply(&mut self, data: Self::Data, settings: Self::Data) {
        // SAFETY: the core and caller own the two live objects, possibly aliased.
        unsafe { obs_data_apply(data, settings) }
    }
    fn addref(&mut self, data: Self::Data) {
        // SAFETY: the core still owns this live object; the caller gets the retain.
        unsafe { obs_data_addref(data) }
    }
}

/// Replace private user values while preserving the core's storage object.
///
/// # Safety
/// The core must be initialized and remain live. `settings` is NULL or a live
/// obs_data_t; all access obeys the existing data API's synchronization contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_set_private_data(settings: *mut c_void) {
    private_data::set(&mut Calls, NonNull::new(settings).map(NonNull::as_ptr));
}

/// Merge `settings` into the core's private data without clearing it.
///
/// # Safety
/// `settings` is NULL or a live obs_data_t. NULL returns before any core
/// access; otherwise the core must be initialized and remain live, and all
/// access obeys the existing data API's synchronization contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_apply_private_data(settings: *mut c_void) {
    private_data::apply(&mut Calls, NonNull::new(settings).map(NonNull::as_ptr));
}

/// Return a retained reference to the core's private data.
///
/// # Safety
/// The core must be initialized and remain live during this call. The caller
/// releases the returned reference with obs_data_release.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_private_data() -> *mut c_void {
    private_data::get(&mut Calls)
}
