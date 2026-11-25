//! Private ABI for the service registry lookup. C retains layouts and storage.
use core::ffi::{CStr, c_char, c_void};

unsafe extern "C" {
    fn obs_service_find_count() -> usize;
    fn obs_service_find_entry(index: usize) -> *const c_void;
    fn obs_service_find_id(entry: *const c_void) -> *const c_char;
}

/// Return the first byte-exact match as a borrowed pointer into the live registry.
///
/// # Safety
/// The core must be initialized. `id` and registered IDs must be valid C strings;
/// the C-owned registry must remain live and unmodified throughout the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn find_service(id: *const c_char) -> *const c_void {
    // SAFETY: the caller preserves the original C lookup preconditions. Each
    // index is below the logical count, and C projects its own native layout.
    unsafe {
        let entries = (0..obs_service_find_count()).filter_map(|index| {
            let entry = obs_service_find_entry(index);
            if entry.is_null() {
                return None;
            }
            Some((CStr::from_ptr(obs_service_find_id(entry)).to_bytes(), entry))
        });
        crate::service_find::find(CStr::from_ptr(id).to_bytes(), entries)
            .unwrap_or(core::ptr::null())
    }
}
