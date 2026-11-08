//! Private field-address conversion and atomic loads under the original C contract.
use core::ffi::{CStr, c_char, c_int, c_void};
use core::sync::atomic::{AtomicBool, Ordering};

#[repr(C)]
pub struct Fields {
    pub active: *const bool,
    pub paused: *const bool,
}

unsafe extern "C" {
    fn obs_encoder_state_queries_fields(encoder: *const c_void) -> Fields;
    fn blog(level: c_int, format: *const c_char, ...);
}

fn log(name: &'static CStr) {
    // SAFETY: literal format and static terminated function name.
    unsafe { blog(400, c"%s: Null 'encoder' parameter".as_ptr(), name.as_ptr()) }
}

unsafe fn load(encoder: *const c_void, field: impl FnOnce(Fields) -> *const bool) -> Option<bool> {
    if encoder.is_null() {
        return None;
    }
    // SAFETY: NULL is rejected before C takes member addresses. The caller
    // retains the encoder, and C accesses these bool fields atomically too.
    unsafe {
        let pointer = field(obs_encoder_state_queries_fields(encoder));
        Some(AtomicBool::from_ptr(pointer.cast_mut()).load(Ordering::SeqCst))
    }
}

/// # Safety
/// A non-null encoder must be live with its atomic active flag readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_active(encoder: *const c_void) -> bool {
    // SAFETY: caller retains the encoder; load rejects NULL before field access.
    crate::encoder_state_queries::active(unsafe { load(encoder, |f| f.active) }, log)
}

/// # Safety
/// A non-null encoder must be live with its atomic paused flag readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_paused(encoder: *const c_void) -> bool {
    // SAFETY: caller retains the encoder; load rejects NULL before field access.
    crate::encoder_state_queries::paused(unsafe { load(encoder, |f| f.paused) }, log)
}
