//! C ABI shim for `os_generate_formatted_filename` from
//! `libobs/util/platform-filename.c` (`libobs/util/platform.h`), over the
//! safe core in [`crate::platform_filename`].
//!
//! As in C, the shim reads the time (`time`, then `localtime`) and then the
//! video info from libobs's `obs_get_video_info`, once per call and before
//! the format is looked at, without checking its result (characterized, not
//! endorsed; #1509). The C reads fields the call did not write from its
//! uninitialized `struct obs_video_info`; Rust cannot, so the struct starts
//! zeroed. The name is `bmalloc`ed, or NULL when it is empty.

use core::ffi::{CStr, c_char, c_int};
use core::ptr;

use crate::ffi::platform_file::to_bmalloc;
use crate::platform_filename::{self, Now, VideoInfo};

/// `struct obs_video_info` from `libobs/obs.h`. The enums are `c_int`s.
#[repr(C)]
#[allow(non_camel_case_types)] // the C name
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct obs_video_info {
    pub graphics_module: *const c_char,
    pub fps_num: u32,
    pub fps_den: u32,
    pub base_width: u32,
    pub base_height: u32,
    pub output_width: u32,
    pub output_height: u32,
    pub output_format: c_int,
    pub adapter: u32,
    pub gpu_conversion: bool,
    pub colorspace: c_int,
    pub range: c_int,
    pub scale_type: c_int,
}

impl obs_video_info {
    /// All fields zero, `graphics_module` NULL.
    pub const ZERO: Self = Self {
        graphics_module: ptr::null(),
        fps_num: 0,
        fps_den: 0,
        base_width: 0,
        base_height: 0,
        output_width: 0,
        output_height: 0,
        output_format: 0,
        adapter: 0,
        gpu_conversion: false,
        colorspace: 0,
        range: 0,
        scale_type: 0,
    };
}

impl From<&obs_video_info> for VideoInfo {
    fn from(ovi: &obs_video_info) -> Self {
        Self {
            fps_num: ovi.fps_num,
            fps_den: ovi.fps_den,
            base_width: ovi.base_width,
            base_height: ovi.base_height,
            output_width: ovi.output_width,
            output_height: ovi.output_height,
            output_format: ovi.output_format,
        }
    }
}

unsafe extern "C" {
    /// libobs core (`libobs/obs.c`).
    fn obs_get_video_info(ovi: *mut obs_video_info) -> bool;
}

/// `ptr`'s bytes up to the NUL, or none for NULL.
///
/// # Safety
/// `ptr` is NULL or a valid NUL-terminated string that outlives `'a`.
unsafe fn bytes<'a>(ptr: *const c_char) -> &'a [u8] {
    if ptr.is_null() {
        &[]
    } else {
        // SAFETY: the caller guarantees a valid NUL-terminated string.
        unsafe { CStr::from_ptr(ptr) }.to_bytes()
    }
}

/// # Safety
/// `extension` and `format` are NULL or valid NUL-terminated strings. As in
/// C, libobs must be started (`obs_get_video_info` reads the core).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_generate_formatted_filename(
    extension: *const c_char,
    space: bool,
    format: *const c_char,
) -> *mut c_char {
    let now = Now::current();
    let mut ovi = obs_video_info::ZERO;
    // SAFETY: `ovi` is a writable `struct obs_video_info`. The result is not
    // checked, as in C (#1509).
    let _ = unsafe { obs_get_video_info(&mut ovi) };
    let video = VideoInfo::from(&ovi);

    // SAFETY: the caller guarantees valid strings or NULL.
    let (extension, format) = unsafe { (bytes(extension), bytes(format)) };
    platform_filename::generate_formatted_filename(extension, space, format, &now, &video)
        .map_or(ptr::null_mut(), |name| to_bmalloc(&name))
}
