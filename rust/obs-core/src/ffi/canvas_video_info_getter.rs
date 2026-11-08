//! C retains private layouts; Rust preserves lazy selection and byte copying.
use core::ffi::{c_char, c_void};
use core::ptr::NonNull;

/// Public `obs_video_info` from the unchanged real header.
#[repr(C)]
pub struct VideoInfo {
    pub graphics_module: *const c_char,
    pub fps_num: u32,
    pub fps_den: u32,
    pub base_width: u32,
    pub base_height: u32,
    pub output_width: u32,
    pub output_height: u32,
    pub output_format: i32,
    pub adapter: u32,
    pub gpu_conversion: bool,
    pub colorspace: i32,
    pub range: i32,
    pub scale_type: i32,
}

unsafe extern "C" {
    fn obs_canvas_video_info_graphics_field() -> *const *mut c_void;
    fn obs_canvas_video_info_mix_field(canvas: *const c_void) -> *const *mut c_void;
    fn obs_canvas_video_info_value_field(mix: *const c_void) -> *const VideoInfo;
}

/// # Safety
/// OBS must be initialized. When graphics exists, `canvas` must be live and
/// non-null; when its mix exists, `output` must be writable. Lifetimes and synchronization follow the C getter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_canvas_get_video_info(
    canvas: *const c_void,
    output: *mut VideoInfo,
) -> bool {
    // SAFETY: initialized OBS owns a readable graphics pointer field.
    let graphics = unsafe { *obs_canvas_video_info_graphics_field() };
    let info = crate::canvas_video_info_getter::video_info(!graphics.is_null(), || {
        // SAFETY: only evaluated with graphics; the caller supplies a live canvas.
        NonNull::new(unsafe { *obs_canvas_video_info_mix_field(canvas) }).map(|mix| {
            // SAFETY: a present mix owns readable info throughout this call.
            unsafe { obs_canvas_video_info_value_field(mix.as_ptr()) }
        })
    });
    info.is_some_and(|info| {
        // SAFETY: success requires the caller's writable output. Copying raw
        // storage retains the characterized padding bytes and borrowed pointer.
        unsafe { core::ptr::copy(info, output, 1) };
        true
    })
}
