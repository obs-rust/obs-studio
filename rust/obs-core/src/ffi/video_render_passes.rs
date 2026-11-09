//! Thin callback ABI for the private video render helpers. No C layouts cross it.
use crate::video_render_passes;
use std::ffi::{c_char, c_void};

/// Synchronous graphics adapter. Handles and callback results are pointer-sized.
pub type RenderCall = unsafe extern "C" fn(u32, usize, usize) -> usize;

/// Draw a selected mix through the caller's graphics adapter.
///
/// # Safety
/// The callback must accept the operation protocol in `video_render_passes` and
/// retain all returned graphics handles until this call completes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_video_draw_mix_texture(index: usize, call: RenderCall) {
    video_render_passes::draw_mix_texture(index, |operation, a, b| {
        // SAFETY: the caller supplies a synchronous adapter for this protocol.
        unsafe { call(operation as u32, a, b) }
    });
}

/// Convert a plane through the caller's graphics adapter.
///
/// # Safety
/// Effect, target and name must remain valid for the callback's graphics calls.
/// The callback follows the same lifetime and operation contract as above.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_video_render_convert_plane(
    effect: *mut c_void,
    target: *mut c_void,
    name: *const c_char,
    call: RenderCall,
) {
    video_render_passes::render_convert_plane(
        effect as usize,
        target as usize,
        name as usize,
        |operation, a, b| {
            // SAFETY: the caller supplies a synchronous adapter for this protocol.
            unsafe { call(operation as u32, a, b) }
        },
    );
}
