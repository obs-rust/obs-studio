//! Borrow the main canvas and preserve the existing getter's acquired result.
use core::ffi::c_void;

/// Private C layout and existing channel operation, supplied together.
#[repr(C)]
pub struct Fields {
    pub canvas: *mut c_void,
    pub get_channel: unsafe extern "C" fn(*mut c_void, u32) -> *mut c_void,
}

unsafe extern "C" {
    fn obs_get_output_source_fields() -> Fields;
}

/// # Safety
/// The core must be initialized and its current main canvas live. The caller
/// observes the original canvas getter's channel preconditions and releases
/// the returned source reference with `obs_source_release` when non-NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_output_source(channel: u32) -> *mut c_void {
    // SAFETY: the caller preserves the initialized-core precondition. The C
    // adapter reads the borrowed main canvas and returns the existing getter.
    let fields = unsafe { obs_get_output_source_fields() };
    crate::get_output_source::get_output_source(fields.canvas, channel, |c, ch| {
        // SAFETY: these are the unchanged canvas/channel. The existing getter
        // owns channel assertions, locking and the acquired source reference.
        unsafe { (fields.get_channel)(c, ch) }
    })
}
