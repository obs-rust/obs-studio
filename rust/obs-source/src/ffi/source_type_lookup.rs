//! Shared binding to the existing source-info lookup; ID policy belongs to it.
use core::ffi::{c_char, c_void};

unsafe extern "C" {
    /// # Safety
    /// The core and its registry are live and synchronized. The ID satisfies
    /// the original lookup contract, including readability when nonempty.
    pub fn get_source_info(id: *const c_char) -> *const c_void;
}
