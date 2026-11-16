//! Shared binding to libobs's existing encoder registry lookup; no traversal here.
use core::ffi::{c_char, c_void};

unsafe extern "C" {
    /// # Safety
    /// The core must be live and its registry synchronized. The ID must be a
    /// readable C string whenever the registry is nonempty; NULL is only valid
    /// for an empty registry, exactly as in the original C lookup.
    pub fn find_encoder(id: *const c_char) -> *const c_void;
}
