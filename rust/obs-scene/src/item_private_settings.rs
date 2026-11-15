//! The scene item private-settings getter: the stored object with one added
//! reference per call, or the original NULL diagnostic.
use core::ffi::CStr;

/// The function name the original `obs_ptr_valid` diagnostic reports.
pub const NAME: &CStr = c"obs_sceneitem_get_private_settings";

/// Return the stored settings after exactly one `addref`, or log `NAME` and
/// return the default (NULL) when there is no item. A stored NULL is
/// returned unchanged and still passed to `addref`, as in C.
pub fn get<T: Copy + Default>(
    stored: Option<T>,
    addref: impl FnOnce(T),
    log: impl FnOnce(&'static CStr),
) -> T {
    let _ = (stored, addref, log);
    todo!("obs_sceneitem_get_private_settings core")
}
