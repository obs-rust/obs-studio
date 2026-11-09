//! Borrow the real C counter address and read it atomically.
use core::ffi::{CStr, c_char, c_int, c_long, c_void};
use core::sync::atomic::{AtomicI32, AtomicI64, Ordering};

#[repr(C)]
pub struct Fields {
    pub frames: *const c_long,
}

unsafe extern "C" {
    fn obs_output_total_frames_fields(output: *const c_void) -> Fields;
    fn blog(level: c_int, format: *const c_char, ...);
}

fn log(name: &'static CStr) {
    // SAFETY: literal format and static NUL-terminated function name.
    unsafe { blog(400, c"%s: Null 'output' parameter".as_ptr(), name.as_ptr()) }
}

/// # Safety
/// A non-null output must remain live, with its C long counter aligned for
/// atomic access. C writers use the same-width atomic operations.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_total_frames(output: *const c_void) -> c_int {
    // SAFETY: adapter borrows the counter address, or returns a null field.
    let fields = unsafe { obs_output_total_frames_fields(output) };
    let value = if fields.frames.is_null() {
        None
    } else if size_of::<c_long>() == size_of::<i64>() {
        // SAFETY: live aligned 64-bit C long, shared only through atomic accesses.
        Some(unsafe { AtomicI64::from_ptr(fields.frames.cast_mut().cast()) }.load(Ordering::SeqCst))
    } else {
        // SAFETY: live aligned 32-bit C long, shared only through atomic accesses.
        Some(i64::from(
            unsafe { AtomicI32::from_ptr(fields.frames.cast_mut().cast()) }.load(Ordering::SeqCst),
        ))
    };
    crate::output_total_frames::get(value, log)
}
