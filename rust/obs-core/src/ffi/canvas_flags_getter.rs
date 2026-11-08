//! Borrow the stored flags through the real C canvas layout.
use core::ffi::c_void;

/// Private field view supplied by the C adapter.
#[repr(C)]
pub struct Fields {
    pub flags: *const u32,
}

unsafe extern "C" {
    fn obs_canvas_flags_fields(canvas: *const c_void) -> Fields;
}

/// # Safety
/// `canvas` must be non-null and live, with its flags readable under the
/// same synchronization rules as the original C getter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_canvas_get_flags(canvas: *const c_void) -> u32 {
    // SAFETY: the caller retains a live canvas; C borrows its flags field.
    let fields = unsafe { obs_canvas_flags_fields(canvas) };
    // SAFETY: the adapter returns the readable uint32_t field of that canvas.
    crate::canvas_flags_getter::stored_flags(unsafe { &*fields.flags })
}

/// # Safety
/// `canvas` must be non-null and live, with its flags readable under the
/// same synchronization rules as the original C query.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_canvas_removed(canvas: *mut c_void) -> bool {
    // SAFETY: the caller retains a live canvas; C borrows its flags field.
    let fields = unsafe { obs_canvas_flags_fields(canvas) };
    // SAFETY: the adapter returns the readable uint32_t field of that canvas.
    crate::canvas_flags_getter::stored_removed(unsafe { &*fields.flags })
}
