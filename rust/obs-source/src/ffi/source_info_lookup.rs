//! `get_source_info()` over the core's source type registry. The entries stay
//! C: the private adapter in `libobs/obs-source-info-lookup-fields.c.h` gives
//! the count, an entry's address and its ID, and the shim returns the C
//! entry's own address, which callers keep.
use crate::info_lookup::{self, Registry};
use core::ffi::{CStr, c_char, c_void};

unsafe extern "C" {
    fn obs_source_info_lookup_count() -> usize;
    fn obs_source_info_lookup_entry(index: usize) -> *mut c_void;
    fn obs_source_info_lookup_id(info: *const c_void) -> *const c_char;
}

/// The live `obs->source_types` array, read through the adapter.
struct Core;

impl Registry for Core {
    fn count(&mut self) -> usize {
        // SAFETY: get_source_info's caller guarantees an initialized core,
        // as the C did; the adapter only reads `source_types.num`.
        unsafe { obs_source_info_lookup_count() }
    }

    fn id(&mut self, index: usize) -> Option<&[u8]> {
        // SAFETY: `index` is below the count just read, so the entry is a
        // live element of `source_types`; its ID is NULL or a NUL-terminated
        // string the registry owns for as long as the type is registered.
        unsafe {
            let id = obs_source_info_lookup_id(obs_source_info_lookup_entry(index));
            (!id.is_null()).then(|| CStr::from_ptr(id).to_bytes())
        }
    }
}

/// # Safety
/// OBS must be initialized, as for the C, and `id` must be NULL or a
/// readable NUL-terminated string. Unlike the C, a NULL `id` finds nothing.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn get_source_info(id: *const c_char) -> *mut c_void {
    // SAFETY: a non-NULL `id` is a NUL-terminated string (contract above).
    let id = (!id.is_null()).then(|| unsafe { CStr::from_ptr(id) }.to_bytes());
    match info_lookup::find(&mut Core, id) {
        // SAFETY: `find` returned an index below the live count.
        Some(index) => unsafe { obs_source_info_lookup_entry(index) },
        None => core::ptr::null_mut(),
    }
}
