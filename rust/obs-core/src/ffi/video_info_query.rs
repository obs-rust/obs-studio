//! Private C layouts remain at field adapters; selection lives in the safe core.
use core::ffi::c_void;
use core::ptr::NonNull;

pub use super::canvas_video_info_getter::VideoInfo;

unsafe extern "C" {
    fn obs_video_info_query_core() -> *const c_void;
    fn obs_video_info_query_graphics_field(core: *const c_void) -> *const *mut c_void;
    fn obs_video_info_query_mix_field(core: *const c_void) -> *const *mut c_void;
    fn obs_video_info_query_value_field(mix: *const c_void) -> *const VideoInfo;
}

/// # Safety
/// OBS state must be synchronized as for the C getter. With graphics, the main
/// canvas must be live; with its mix, `output` must be writable. A NULL output
/// is accepted only when the core, graphics or mix is unavailable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_video_info(output: *mut VideoInfo) -> bool {
    // SAFETY: reading the global pointer forms no address through its value.
    let core = NonNull::new(unsafe { obs_video_info_query_core() }.cast_mut());
    let info = crate::video_info_query::video_info(
        core,
        |core| {
            // SAFETY: the safe core invokes this only after the NULL core gate.
            !unsafe { *obs_video_info_query_graphics_field(core.as_ptr()) }.is_null()
        },
        |core| {
            // SAFETY: core and graphics are present; the C precondition requires
            // a live main canvas. No main-canvas field is read before graphics.
            NonNull::new(unsafe { *obs_video_info_query_mix_field(core.as_ptr()) }).map(|mix| {
                // SAFETY: the present mix owns a readable stored video-info value.
                unsafe { obs_video_info_query_value_field(mix.as_ptr()) }
            })
        },
    );
    info.is_some_and(|info| {
        // SAFETY: success requires a writable output. Raw copy preserves the
        // complete struct including borrowed pointer and padding; aliasing is OK.
        unsafe { core::ptr::copy(info, output, 1) };
        true
    })
}
