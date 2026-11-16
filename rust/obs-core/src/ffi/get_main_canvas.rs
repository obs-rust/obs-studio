//! Preserve the original wrapper's initialized-core and ownership contract.
use core::ffi::c_void;

/// Private C field and the existing canvas reference operation.
#[repr(C)]
pub struct Fields {
    pub canvas: *mut c_void,
    pub get_ref: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
}

unsafe extern "C" {
    fn obs_get_main_canvas_fields() -> Fields;
}

/// # Safety
/// The core must be initialized. Its main canvas is NULL or live under the
/// original getter's synchronization and reference-count preconditions.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_main_canvas() -> *mut c_void {
    // SAFETY: the caller preserves the original initialized-core precondition;
    // the adapter borrows the current field without acquiring ownership.
    let fields = unsafe { obs_get_main_canvas_fields() };
    crate::get_main_canvas::get_main_canvas(fields.canvas, |canvas| {
        // SAFETY: forward the unchanged canvas exactly once to the existing
        // reference operation, and return its acquired result unchanged.
        unsafe { (fields.get_ref)(canvas) }
    })
}
