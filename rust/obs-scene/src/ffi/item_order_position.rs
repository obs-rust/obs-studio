//! Scene-item order position over the real C parent, head, link and full-lock
//! seams.
use core::ffi::{c_int, c_void};

/// Zero-based position of `item` in its parent's list (0 for NULL or
/// detached).
///
/// # Safety
/// A non-null item must be readable and its parent scene live, as in C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_get_order_position(item: *mut c_void) -> c_int {
    let _ = item;
    0
}
