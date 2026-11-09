//! Private ABI: C owns the video layout; Rust only borrows effect identities.
use core::ffi::c_void;
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_video_scale_effect_fields(video: *const c_void, effects: *mut *mut c_void);
}

/// # Safety
/// `video` is non-null, live and readable under the original caller's synchronization.
unsafe fn effects(video: *const c_void) -> [Option<NonNull<c_void>>; 5] {
    assert!(!video.is_null(), "scale-effect video must be live");
    let mut effects = [core::ptr::null_mut(); 5];
    // SAFETY: the caller provides a live video; C writes exactly five pointers.
    unsafe { obs_video_scale_effect_fields(video, effects.as_mut_ptr()) };
    effects.map(NonNull::new)
}

/// # Safety
/// `video` is non-null and live with its effect fields readable, just as in C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_video_loaded_scale_effects(video: *const c_void) -> u32 {
    // SAFETY: forwarded original C helper's non-null/live contract.
    crate::video_scale_effects::loaded(&unsafe { effects(video) })
}

/// # Safety
/// `video` is non-null and live with its effect fields readable. The returned
/// pointer is borrowed; this neither acquires a reference nor a lock.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_video_scale_effect_pointer(
    video: *const c_void,
    selector: i32,
) -> *mut c_void {
    // SAFETY: forwarded original C helper's non-null/live contract.
    crate::video_scale_effects::selected(&unsafe { effects(video) }, selector)
        .map_or(core::ptr::null_mut(), NonNull::as_ptr)
}
