//! Borrow the main canvas without acquiring a canvas or source reference.
use core::ffi::c_void;

/// Private C layout and existing channel operation, supplied together.
#[repr(C)]
pub struct Fields {
    pub canvas: *mut c_void,
    pub set_channel: unsafe extern "C" fn(*mut c_void, u32, *mut c_void),
}

unsafe extern "C" {
    fn obs_set_output_source_fields() -> Fields;
}

/// # Safety
/// The core must be initialized and its current main canvas live. `source` is
/// NULL or a live source under the original canvas setter's synchronization rules.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_set_output_source(channel: u32, source: *mut c_void) {
    // SAFETY: the caller preserves the initialized-core precondition. The C
    // adapter reads the borrowed main canvas and returns the existing setter.
    let fields = unsafe { obs_set_output_source_fields() };
    crate::set_output_source::set_output_source(fields.canvas, channel, source, |c, ch, s| {
        // SAFETY: these are the unchanged canvas, channel and source. C owns
        // bounds assertions, locking, source ownership and mutable signals.
        unsafe { (fields.set_channel)(c, ch, s) };
    });
}
