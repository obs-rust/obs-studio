//! Thin C ABI shim for the borrowed core audio output.
use core::ffi::c_void;

/// Private adapter layout measured against the real C declaration.
#[repr(C)]
pub struct Fields {
    pub audio: *mut c_void,
}

unsafe extern "C" {
    fn obs_get_audio_fields() -> Fields;
}

/// # Safety
/// The core must be initialized. The result is borrowed from the core and
/// must not be closed by the caller; reset/shutdown may invalidate it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_audio() -> *mut c_void {
    // SAFETY: the caller preserves the original initialized-core precondition.
    let fields = unsafe { obs_get_audio_fields() };
    crate::get_audio::get_audio(fields.audio)
}
