//! One borrowed entry point to the existing output-type registry lookup.
use core::ffi::{c_char, c_void};
use core::ptr::NonNull;
unsafe extern "C" {
    fn find_output(id: *const c_char) -> *mut c_void;
}
/// NULL IDs return before accessing the core. Other IDs require a live core and
/// are read only when a registered ID is compared, as in the original C.
pub(super) unsafe fn find(id: *const c_char) -> Option<NonNull<c_void>> {
    // SAFETY: the ABI caller supplies the original lookup contract.
    NonNull::new(unsafe { find_output(id) })
}
