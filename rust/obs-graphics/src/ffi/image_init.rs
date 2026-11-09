//! Opaque FFmpeg session bridge. No FFmpeg layout crosses this boundary.
use crate::image_init::{self, Backend, Operation};
use std::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_ffmpeg_image_operation(
        info: *mut c_void,
        op: u32,
        value: c_int,
        extra: *mut c_void,
        get_buffer: unsafe extern "C" fn(*mut c_void, *mut c_void, c_int) -> c_int,
        max_size: Option<unsafe extern "C" fn() -> u32>,
    ) -> c_int;
}
struct Session {
    info: *mut c_void,
    max_size: Option<unsafe extern "C" fn() -> u32>,
    extra: *mut c_void,
}
impl Backend for Session {
    fn run(&mut self, operation: Operation, value: i32) -> i32 {
        // SAFETY: the private C caller owns the live session and operation-specific extra data.
        unsafe {
            obs_ffmpeg_image_operation(
                self.info,
                operation as u32,
                value,
                self.extra,
                obs_ffmpeg_image_get_buffer,
                self.max_size,
            )
        }
    }
}

/// # Safety
/// `info` is a writable private ffmpeg_image; file is null or a live C path.
/// `max_size` is the caller's live texture-limit query callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ffmpeg_image_init(
    info: *mut c_void,
    file: *const c_char,
    max_size: unsafe extern "C" fn() -> u32,
) -> bool {
    // SAFETY: a non-null file points to at least the C string terminator.
    let nonempty = !file.is_null() && unsafe { *file != 0 };
    image_init::init(
        &mut Session {
            info,
            max_size: Some(max_size),
            extra: file.cast_mut().cast(),
        },
        nonempty,
    )
}

/// # Safety
/// `info` is a live initialized private ffmpeg_image, including partial initialization.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ffmpeg_image_free(info: *mut c_void) {
    image_init::free(&mut Session {
        info,
        max_size: None,
        extra: std::ptr::null_mut(),
    });
}

/// # Safety
/// `info` is live; `buffer` points to the private adapter packet retaining the
/// exact callback context and writable frame for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_ffmpeg_image_get_buffer(
    info: *mut c_void,
    buffer: *mut c_void,
    flags: c_int,
) -> c_int {
    image_init::get_buffer(
        &mut Session {
            info,
            max_size: None,
            extra: buffer,
        },
        flags,
    )
}
