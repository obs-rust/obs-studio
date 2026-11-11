//! Unchanged C ABI for live module enumeration; private layouts stay in C.
use super::module_lists::{Libobs, Seam};
use core::ffi::c_void;

/// The void C callback cannot request early termination.
pub type Callback = Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>;

/// Visits borrowed module pointers with the exact supplied parameter.
///
/// # Safety
/// The core and visited nodes must remain alive, without concurrent mutation.
/// With a nonempty list, `callback` must be non-NULL and callable with `param`.
/// Nested calls and prepending nodes from the callback are permitted.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_enum_modules(callback: Callback, param: *mut c_void) {
    // SAFETY: caller guarantees live nodes/core across each seam access.
    let mut seam = unsafe { Seam::<Libobs>::new() };
    crate::enum_modules::enumerate(&mut seam, |_, module| {
        // SAFETY: callback is required only when a node is visited, as in C.
        // No-core and empty-list paths never inspect or invoke it.
        unsafe { callback.unwrap_unchecked()(param, module.as_ptr()) };
    });
}
