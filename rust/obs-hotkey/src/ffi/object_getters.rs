//! C ABI for read-only hotkey fields. The hotkey itself stays opaque.
use super::pair::obs_hotkey_t;
use crate::object_getters as core;
use std::ffi::{c_char, c_uint, c_void};

/// Private field-pointer seam, measured against obs-internal.h.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
pub struct obs_hotkey_object_fields {
    pub description: *const *mut c_char,
    pub registerer_type: *const c_uint,
    pub registerer: *const *mut c_void,
    pub pair_partner_id: *const usize,
}

unsafe extern "C" {
    fn obs_hotkey_object_fields(key: *const obs_hotkey_t) -> obs_hotkey_object_fields;
}

/// # Safety
/// `key` is a live hotkey whose description field is initialized.
/// The stored description may be null and its bytes are not read.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_get_description(key: *const obs_hotkey_t) -> *const c_char {
    // SAFETY: C supplies pointers into the live hotkey; only this field is read.
    unsafe { core::description(&*obs_hotkey_object_fields(key).description).cast_const() }
}

/// # Safety
/// `key` is a live hotkey whose registerer_type field is initialized.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_get_registerer_type(key: *const obs_hotkey_t) -> c_uint {
    // SAFETY: C supplies pointers into the live hotkey; only this field is read.
    unsafe { core::registerer_type(&*obs_hotkey_object_fields(key).registerer_type) }
}

/// # Safety
/// `key` is a live hotkey whose registerer field is initialized.
/// The stored registerer may be null; it is neither dereferenced nor retained.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_get_registerer(key: *const obs_hotkey_t) -> *mut c_void {
    // SAFETY: C supplies pointers into the live hotkey; only this field is read.
    unsafe { core::registerer(&*obs_hotkey_object_fields(key).registerer) }
}

/// # Safety
/// `key` is a live hotkey whose pair_partner_id field is initialized.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_get_pair_partner_id(key: *const obs_hotkey_t) -> usize {
    // SAFETY: C supplies pointers into the live hotkey; only this field is read.
    unsafe { core::pair_partner_id(&*obs_hotkey_object_fields(key).pair_partner_id) }
}
