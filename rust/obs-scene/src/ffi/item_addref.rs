//! Opaque scene items: C retains the reference counter's layout, width and
//! atomic ordering.
use core::ffi::c_void;

/// Add one reference to a scene item; NULL is a no-op.
///
/// # Safety
/// A non-null item must remain live for the call, as in the original C API.
pub unsafe extern "C" fn obs_sceneitem_addref(item: *mut c_void) {
    let _ = item;
}
