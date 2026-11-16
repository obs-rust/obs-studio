//! `oracle_*` build of `libobs/obs-video-frame-copy.c.h`
//! (oracle/video_frame_copy.c): the output frame copy, renamed, over the C
//! copy plan of oracle/video_copy_plan.c.
//!
//! The frames and the output info are untyped pointers: callers pass
//! `struct video_frame`, `struct video_data` and `struct video_output_info`
//! as `obs-media-io`'s mirrors lay them out (its Tier 2 layout tests check
//! them against the C).

use core::ffi::c_void;

unsafe extern "C" {
    /// `output` points to a `struct video_frame`, `input` to a `struct
    /// video_data`, `info` to a `struct video_output_info`; the planes hold
    /// what the plan reads and writes.
    pub fn oracle_obs_video_set_gpu_converted_data(
        output: *mut c_void,
        input: *const c_void,
        info: *const c_void,
    );

    /// As [`oracle_obs_video_set_gpu_converted_data`], for plane 0 only.
    pub fn oracle_obs_video_copy_rgbx_frame(
        output: *mut c_void,
        input: *const c_void,
        info: *const c_void,
    );
}
