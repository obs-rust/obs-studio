//! Field-address conversion only; the private source layout stays in C.
use crate::source_identity_getters::{self, Identity};
use core::ffi::{CStr, c_char, c_int, c_void};

#[repr(C)]
pub struct Fields {
    pub uuid: *const *const c_char,
    pub unversioned_id: *const *const c_char,
}

unsafe extern "C" {
    fn obs_source_identity_getter_fields(source: *const c_void) -> Fields;
    fn blog(level: c_int, format: *const c_char, ...);
}

unsafe fn fields(source: *const c_void) -> Option<Identity<*const c_char>> {
    if source.is_null() {
        return None;
    }
    // SAFETY: null is rejected before taking C member addresses; the caller
    // keeps this source and its metadata fields readable throughout the call.
    unsafe {
        let fields = obs_source_identity_getter_fields(source);
        Some(Identity {
            uuid: *fields.uuid,
            unversioned_id: *fields.unversioned_id,
        })
    }
}

fn log(name: &'static CStr) {
    // SAFETY: the format is a literal and name is a static, terminated string.
    unsafe { blog(400, c"%s: Null 'source' parameter".as_ptr(), name.as_ptr()) }
}

/// # Safety
/// A non-null source must remain live and readable under the original C contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_uuid(source: *const c_void) -> *const c_char {
    // SAFETY: the caller supplies a live source or null; fields rejects null first.
    source_identity_getters::uuid(unsafe { fields(source) }, log)
}

/// # Safety
/// A non-null source must remain live and readable under the original C contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_unversioned_id(source: *const c_void) -> *const c_char {
    // SAFETY: the caller supplies a live source or null; fields rejects null first.
    source_identity_getters::unversioned_id(unsafe { fields(source) }, log)
}
