//! Borrowed field addresses supplied by C, without a canvas layout mirror.
use crate::canvas_readonly_getters::{borrowed, has_video};
use core::ffi::{c_char, c_void};
use core::ptr::NonNull;

/// The private C adapter borrows four pointer fields of a live canvas.
#[repr(C)]
pub struct Fields {
    pub name: *const *mut c_char,
    pub uuid: *const *const c_char,
    pub signals: *const *mut c_void,
    pub mix: *const *mut c_void,
}

unsafe extern "C" {
    fn obs_canvas_readonly_fields(canvas: *const c_void) -> Fields;
}

/// # Safety
/// `canvas` must be non-null and live, with its name field readable under
/// the same synchronization and lifetime rules as the original C getter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_canvas_get_name(canvas: *const c_void) -> *const c_char {
    // SAFETY: the caller retains the canvas; the adapter borrows its fields.
    let fields = unsafe { obs_canvas_readonly_fields(canvas) };
    // SAFETY: name points to the readable pointer field, not its string.
    borrowed(unsafe { &*fields.name }).cast_const()
}

/// # Safety
/// `canvas` must be non-null and live, with its UUID field readable. The
/// returned pointer remains borrowed under the original C lifetime rules.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_canvas_get_uuid(canvas: *const c_void) -> *const c_char {
    // SAFETY: the caller retains the canvas; the adapter borrows its fields.
    let fields = unsafe { obs_canvas_readonly_fields(canvas) };
    // SAFETY: uuid points to the readable pointer field, not its string.
    borrowed(unsafe { &*fields.uuid })
}

/// # Safety
/// `canvas` must be non-null and live, with its signal field readable.
/// The result is borrowed and may be null, as in C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_canvas_get_signal_handler(canvas: *mut c_void) -> *mut c_void {
    // SAFETY: the caller retains the canvas; the adapter borrows its fields.
    let fields = unsafe { obs_canvas_readonly_fields(canvas) };
    // SAFETY: signals points to the readable pointer field.
    borrowed(unsafe { &*fields.signals })
}

/// # Safety
/// `canvas` must be non-null and live, with its mix pointer readable under
/// the synchronization required by the original C getter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_canvas_has_video(canvas: *mut c_void) -> bool {
    // SAFETY: the caller retains the canvas; the adapter borrows its fields.
    let fields = unsafe { obs_canvas_readonly_fields(canvas) };
    // SAFETY: mix points to the readable pointer field. Its pointee is not read.
    has_video(NonNull::new(unsafe { *fields.mix }))
}
