//! C ABI for `libobs/obs-service-info-accessors.c.h` (#3840): the hidden
//! `obs_service_set_info`, `obs_service_set_plugin_data`,
//! `obs_service_get_info_table` and `obs_service_get_plugin_data`
//! (`obs-internal.h`, listed in `libobs/cmake/rust-hidden.d`).
//!
//! Field-address conversion only: `struct obs_service` stays private to C,
//! and `obs_service_info_accessors_fields()` hands back the addresses of the
//! service's own table copy and its plugin data. The service is never NULL,
//! as in the C.

use core::ffi::c_void;
use core::ptr;

use super::service::ObsServiceInfo;
use crate::service_info_accessors;

/// `struct obs_service_info_accessors_fields` (`obs-internal.h`).
#[repr(C)]
pub struct Fields {
    pub info: *mut ObsServiceInfo,
    pub data: *mut *mut c_void,
}

unsafe extern "C" {
    fn obs_service_info_accessors_fields(service: *mut c_void) -> Fields;
}

/// # Safety
///
/// `service` is a live, non-NULL `obs_service_t`.
unsafe fn fields(service: *const c_void) -> Fields {
    // SAFETY: a live service (caller contract); the adapter only takes the
    // addresses of two of its members and neither reads nor writes them.
    unsafe { obs_service_info_accessors_fields(service.cast_mut()) }
}

/// # Safety
///
/// `service` is a live, non-NULL `obs_service_t` and `info` a readable
/// `struct obs_service_info`, as in the C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_service_set_info(service: *mut c_void, info: *const ObsServiceInfo) {
    // SAFETY: the caller's table is read whole before the service is
    // written, so a table that is the service's own copy is copied onto
    // itself unchanged, as the C struct assignment does; the table field is
    // the service's own, writable for this call.
    unsafe {
        let copy = ptr::read(info);
        service_info_accessors::set_info(&mut *fields(service).info, copy);
    }
}

/// # Safety
///
/// `service` is a live, non-NULL `obs_service_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_service_set_plugin_data(service: *mut c_void, data: *mut c_void) {
    // SAFETY: the data field is the service's own, writable for this call;
    // the pointer is stored, never dereferenced.
    unsafe { service_info_accessors::set_plugin_data(&mut *fields(service).data, data) }
}

/// # Safety
///
/// `service` is a live, non-NULL `obs_service_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_service_get_info_table(
    service: *const c_void,
) -> *const ObsServiceInfo {
    // SAFETY: the table field lives as long as the service; only its
    // address is handed back.
    unsafe { service_info_accessors::info_table(&*fields(service).info) }
}

/// # Safety
///
/// `service` is a live, non-NULL `obs_service_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_service_get_plugin_data(service: *const c_void) -> *mut c_void {
    // SAFETY: the data field is the service's own and readable; the pointer
    // is returned as stored, never dereferenced.
    unsafe { service_info_accessors::plugin_data(&*fields(service).data) }
}
