//! `os_generate_formatted_filename` from `libobs/util/platform-filename.c`,
//! compiled unmodified as `oracle_os_generate_formatted_filename` by
//! `oracle/platform.c`.
//!
//! Its `time()` is `oracle_platform_filename_set_time`'s value, and its
//! `obs_get_video_info` a fixture that copies the struct given to
//! `oracle_platform_filename_set_video_info` and returns the given result.
//! The plain `obs_get_video_info`, which the Rust shim calls, reads the same
//! fixture; both count in `oracle_platform_filename_video_info_calls`.
//! `ovi` is a `struct obs_video_info`; the layout functions give its C
//! layout. The fixture and clock are statics: callers must not set them from
//! two threads at once.

use core::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    pub fn oracle_os_generate_formatted_filename(
        extension: *const c_char,
        space: bool,
        format: *const c_char,
    ) -> *mut c_char;
    pub fn oracle_platform_filename_set_time(now: i64);
    pub fn oracle_platform_filename_set_video_info(ovi: *const c_void, result: bool);
    pub fn oracle_platform_filename_video_info_calls() -> c_int;
    pub fn oracle_obs_video_info_size() -> usize;
    pub fn oracle_obs_video_info_align() -> usize;
    pub fn oracle_obs_video_info_offsets(out: *mut usize);
}
