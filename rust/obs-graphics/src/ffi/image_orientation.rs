//! Private FFmpeg adapter calls, without duplicating native frame layouts.
use core::ffi::{c_char, c_int, c_void};

pub type Metadata = unsafe extern "C" fn(*const c_void) -> *const c_char;
pub type Matrix = unsafe extern "C" fn(*const c_void, *mut usize) -> *const u8;
pub type Convert = unsafe extern "C" fn(*const i32) -> c_int;

/// Select the orientation of one live decoded frame.
///
/// # Safety
/// `frame` must satisfy each callback's contract. Metadata is null or a live
/// NUL-terminated string. Matrix returns null or at least `size` readable bytes.
/// Callbacks and their returned storage remain valid until this call returns.
/// `convert` reads exactly nine aligned i32s synchronously and retains no pointer.
pub unsafe fn select_orientation(
    frame: *const c_void,
    metadata: Option<Metadata>,
    matrix: Matrix,
    convert: Convert,
) -> c_int {
    crate::image_orientation::orientation(
        || {
            let metadata = metadata?;
            // SAFETY: caller guarantees the live frame and callback contract.
            let value = unsafe { metadata(frame) };
            if value.is_null() {
                None
            } else {
                // SAFETY: callback returned a live C string. Using the same CRT
                // atoi preserves whitespace, sign and nonnumeric legacy values.
                Some(unsafe { libc::atoi(value) })
            }
        },
        || {
            let mut size = 0;
            // SAFETY: size is writable; caller guarantees the frame contract.
            let data = unsafe { matrix(frame, &mut size) };
            if data.is_null() || size < 36 {
                return None;
            }
            // SAFETY: callback guarantees size readable bytes; only the first
            // 36 are borrowed, avoiding alignment and oversized slice concerns.
            crate::image_orientation::matrix_bytes(unsafe { core::slice::from_raw_parts(data, 36) })
        },
        |matrix| {
            // SAFETY: local array is aligned and lives through the conversion.
            unsafe { convert(matrix.as_ptr()) }
        },
    )
}

unsafe extern "C" {
    fn gs_image_orientation_metadata(frame: *const c_void) -> *const c_char;
    fn gs_image_orientation_matrix(frame: *const c_void, size: *mut usize) -> *const u8;
    fn gs_image_orientation_convert(matrix: *const i32) -> c_int;
}

/// Read the orientation of a decoded FFmpeg frame through the private adapter.
///
/// # Safety
/// A nonnull `frame` must be a live AVFrame for the linked FFmpeg version.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_ffmpeg_image_get_orientation(frame: *const c_void) -> c_int {
    if frame.is_null() {
        return 0;
    }
    // SAFETY: frame is checked before the adapter accesses any field; the
    // caller guarantees its lifetime. Adapter data is borrowed synchronously.
    unsafe {
        select_orientation(
            frame,
            Some(gs_image_orientation_metadata),
            gs_image_orientation_matrix,
            gs_image_orientation_convert,
        )
    }
}
