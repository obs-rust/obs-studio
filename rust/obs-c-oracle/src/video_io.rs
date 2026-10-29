//! `libobs/media-io/video-io.c` under `oracle_` renames.

use core::ffi::{c_char, c_int, c_void};

/// Opaque `video_t` from the oracle build.
pub enum OracleVideo {}

/// Independent `struct video_data`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct OracleVideoData {
    pub data: [*mut u8; 8],
    pub linesize: [u32; 8],
    pub timestamp: u64,
}

/// Independent `struct video_output_info`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct OracleVideoOutputInfo {
    pub name: *const c_char,
    pub format: c_int,
    pub fps_num: u32,
    pub fps_den: u32,
    pub width: u32,
    pub height: u32,
    pub cache_size: usize,
    pub colorspace: c_int,
    pub range: c_int,
}

/// Independent `struct video_scale_info`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct OracleVideoScaleInfo {
    pub format: c_int,
    pub width: u32,
    pub height: u32,
    pub range: c_int,
    pub colorspace: c_int,
}

/// Independent `struct video_frame`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct OracleVideoFrame {
    pub data: [*mut u8; 8],
    pub linesize: [u32; 8],
}

unsafe extern "C" {
    pub fn oracle_video_output_open(
        video: *mut *mut OracleVideo,
        info: *mut OracleVideoOutputInfo,
    ) -> c_int;
    pub fn oracle_video_output_close(video: *mut OracleVideo);
    pub fn oracle_video_output_connect(
        video: *mut OracleVideo,
        conversion: *const OracleVideoScaleInfo,
        callback: Option<unsafe extern "C" fn(*mut c_void, *mut OracleVideoData)>,
        param: *mut c_void,
    ) -> bool;
    pub fn oracle_video_output_connect2(
        video: *mut OracleVideo,
        conversion: *const OracleVideoScaleInfo,
        frame_rate_divisor: u32,
        callback: Option<unsafe extern "C" fn(*mut c_void, *mut OracleVideoData)>,
        param: *mut c_void,
    ) -> bool;
    pub fn oracle_video_output_disconnect2(
        video: *mut OracleVideo,
        callback: Option<unsafe extern "C" fn(*mut c_void, *mut OracleVideoData)>,
        param: *mut c_void,
    ) -> bool;
    pub fn oracle_video_output_align_frame_rate_divisor(
        video: *mut OracleVideo,
        start_timestamp: u64,
        callback: Option<unsafe extern "C" fn(*mut c_void, *mut OracleVideoData)>,
        param: *mut c_void,
    ) -> bool;
    pub fn oracle_video_output_get_info(video: *const OracleVideo) -> *const OracleVideoOutputInfo;
    pub fn oracle_video_output_lock_frame(
        video: *mut OracleVideo,
        frame: *mut OracleVideoFrame,
        count: c_int,
        timestamp: u64,
    ) -> bool;
    pub fn oracle_video_output_unlock_frame(video: *mut OracleVideo);
    pub fn oracle_video_output_get_frame_time(video: *const OracleVideo) -> u64;
    pub fn oracle_video_output_stop(video: *mut OracleVideo);
    pub fn oracle_video_output_get_frame_rate(video: *const OracleVideo) -> f64;
    pub fn oracle_video_output_get_total_frames(video: *const OracleVideo) -> u32;
    pub fn oracle_video_output_get_width(video: *const OracleVideo) -> u32;
    pub fn oracle_video_output_get_format(video: *const OracleVideo) -> c_int;
    pub fn oracle_video_output_create_with_frame_rate_divisor(
        video: *mut OracleVideo,
        divisor: u32,
    ) -> *mut OracleVideo;
    pub fn oracle_video_output_free_frame_rate_divisor(video: *mut OracleVideo);
}
