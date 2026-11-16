//! Scene color-space callback; the private video-info layout stays in C.
use core::ffi::{c_int, c_void};

unsafe extern "C" {
    fn obs_scene_video_color_space_query(colorspace: *mut c_int) -> bool;
}

/// The scene callback ignores data, count and the preferred-space pointer.
#[unsafe(no_mangle)]
pub extern "C" fn scene_video_get_color_space(
    _data: *mut c_void,
    _count: usize,
    _preferred_spaces: *const c_int,
) -> c_int {
    let mut colorspace = 0;
    // SAFETY: the adapter writes one initialized int on success, never retains it.
    let present = unsafe { obs_scene_video_color_space_query(&mut colorspace) };
    crate::color_space::video_color_space(present.then_some(colorspace))
}
