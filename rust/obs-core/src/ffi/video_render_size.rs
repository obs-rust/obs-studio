//! Private C ABI: adapt graphics callbacks to the safe render-size core.
use crate::video_render_size::{self, Graphics};
use core::ffi::c_int;

struct Callbacks {
    depth: unsafe extern "C" fn(bool),
    cull: unsafe extern "C" fn(c_int),
    ortho: unsafe extern "C" fn(f32, f32, f32, f32, f32, f32),
    viewport: unsafe extern "C" fn(c_int, c_int, c_int, c_int),
}
impl Graphics for Callbacks {
    fn depth(&mut self, enabled: bool) {
        // SAFETY: caller supplies a valid graphics callback for this invocation.
        unsafe { (self.depth)(enabled) }
    }
    fn cull(&mut self, mode: i32) {
        // SAFETY: core supplies GS_NEITHER; callback is valid by the ABI contract.
        unsafe { (self.cull)(mode) }
    }
    fn ortho(&mut self, p: [f32; 6]) {
        // SAFETY: caller supplies a valid graphics callback for this invocation.
        unsafe { (self.ortho)(p[0], p[1], p[2], p[3], p[4], p[5]) }
    }
    fn viewport(&mut self, r: [i32; 4]) {
        // SAFETY: caller supplies a valid graphics callback for this invocation.
        unsafe { (self.viewport)(r[0], r[1], r[2], r[3]) }
    }
}

/// Configure rendering with the caller's graphics operations.
///
/// # Safety
/// All callbacks must be valid for the duration of this call, support the
/// current graphics context, and must not unwind across the C boundary.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_video_set_render_size(
    width: u32,
    height: u32,
    depth: unsafe extern "C" fn(bool),
    cull: unsafe extern "C" fn(c_int),
    ortho: unsafe extern "C" fn(f32, f32, f32, f32, f32, f32),
    viewport: unsafe extern "C" fn(c_int, c_int, c_int, c_int),
) {
    video_render_size::set_render_size(
        width,
        height,
        &mut Callbacks {
            depth,
            cull,
            ortho,
            viewport,
        },
    );
}
