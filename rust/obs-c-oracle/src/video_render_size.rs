//! Original C render-size helper with per-call recording callbacks.
use core::ffi::c_int;
unsafe extern "C" {
    pub fn oracle_obs_video_set_render_size(
        width: u32,
        height: u32,
        depth: unsafe extern "C" fn(bool),
        cull: unsafe extern "C" fn(c_int),
        ortho: unsafe extern "C" fn(f32, f32, f32, f32, f32, f32),
        viewport: unsafe extern "C" fn(c_int, c_int, c_int, c_int),
    );
}
