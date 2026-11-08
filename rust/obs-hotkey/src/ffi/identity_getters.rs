//! C ABI for stored hotkey identity fields. The hotkey remains opaque.
use super::pair::obs_hotkey_t;
use crate::identity_getters as core;
use std::ffi::c_char;

/// Borrowed field pointers, measured against the private C header.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
pub struct obs_hotkey_identity_fields {
    pub id: *const usize,
    pub name: *const *mut c_char,
}

unsafe extern "C" {
    fn obs_hotkey_identity_fields(key: *const obs_hotkey_t) -> obs_hotkey_identity_fields;
}

/// # Safety
/// `key` is a live hotkey with an initialized ID field.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_get_id(key: *const obs_hotkey_t) -> usize {
    // SAFETY: C supplies pointers into the live hotkey; only this field is read.
    unsafe { core::id(&*obs_hotkey_identity_fields(key).id) }
}

/// # Safety
/// `key` is a live hotkey with an initialized name field. Its stored name may
/// be null; the pointed-to bytes are neither read nor retained.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_get_name(key: *const obs_hotkey_t) -> *const c_char {
    // SAFETY: C supplies pointers into the live hotkey; only this field is read.
    unsafe { core::name(&*obs_hotkey_identity_fields(key).name).cast_const() }
}
