//! Hidden state accessors over the private C field view.
use crate::service_state as core;
use std::ffi::c_void;

/// Pointers into the original service, measured against `obs-internal.h`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ServiceStateFields {
    pub active: *mut bool,
    pub destroy: *mut bool,
    pub output: *mut *mut c_void,
    pub control: *const *mut c_void,
}
unsafe extern "C" {
    fn obs_service_state_fields(service: *mut c_void) -> ServiceStateFields;
    fn obs_service_output_service_field(output: *mut c_void) -> *mut *mut c_void;
}

/// Mark a service for destruction.
/// # Safety
/// `service` must be live and exclusively accessible, as in C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_service_set_destroy(service: *mut c_void) {
    // SAFETY: the caller provides the live service and exclusive field access.
    unsafe { core::set_destroy(&mut *obs_service_state_fields(service).destroy) }
}
/// Read the destroy flag.
/// # Safety
/// `service` must be live with synchronized access, as in C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_service_is_destroying(service: *const c_void) -> bool {
    // SAFETY: the caller provides a live service with synchronized field access.
    unsafe { core::is_destroying(*obs_service_state_fields(service.cast_mut()).destroy) }
}
/// Read the active flag.
/// # Safety
/// `service` must be live with synchronized access, as in C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_service_is_active(service: *const c_void) -> bool {
    // SAFETY: the caller provides a live service with synchronized field access.
    unsafe { core::is_active(*obs_service_state_fields(service.cast_mut()).active) }
}
/// Set the active flag.
/// # Safety
/// `service` must be live and exclusively accessible, as in C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_service_set_active(service: *mut c_void, active: bool) {
    // SAFETY: the caller provides a live service and exclusive field access.
    unsafe { core::set_active(&mut *obs_service_state_fields(service).active, active) }
}
/// Check whether the output link is present.
/// # Safety
/// `service` must be live with synchronized access, as in C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_service_has_output(service: *const c_void) -> bool {
    // SAFETY: the caller provides a live service with synchronized field access.
    unsafe { core::has_output(*obs_service_state_fields(service.cast_mut()).output) }
}
/// Clear only the service's output link.
/// # Safety
/// `service` must be live and exclusively accessible, as in C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_service_clear_output(service: *mut c_void) {
    // SAFETY: the caller provides a live service and exclusive field access.
    unsafe { core::clear_output(&mut *obs_service_state_fields(service).output) }
}
/// Clear only the output's service link, even if it points at another service.
/// # Safety
/// `service` and its non-null output must be live and exclusively accessible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_service_detach_from_output(service: *mut c_void) {
    // SAFETY: as in C, the caller guarantees a live, non-null output.
    unsafe {
        let output = *obs_service_state_fields(service).output;
        core::detach_from_output(&mut *obs_service_output_service_field(output));
    }
}
/// Borrow the control pointer without acquiring a reference.
/// # Safety
/// `service` must be live with synchronized access, as in C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_service_get_control(service: *const c_void) -> *mut c_void {
    // SAFETY: the caller provides a live service with synchronized field access.
    unsafe { core::get_control(*obs_service_state_fields(service.cast_mut()).control) }
}
