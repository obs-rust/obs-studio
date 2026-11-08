//! Borrow field addresses through C's real layout; video remains borrowed.
use core::ffi::c_void;
use core::ptr::NonNull;

/// Private C view of the stored mix pointer.
#[repr(C)]
pub struct Fields {
    pub mix: *const *mut c_void,
}

unsafe extern "C" {
    fn obs_canvas_video_fields(canvas: *const c_void) -> Fields;
    fn obs_canvas_video_output_field(mix: *const c_void) -> *const *mut c_void;
}

/// # Safety
/// `canvas` must be non-null and live, with its mix and any mix video field
/// readable under the original C getter's synchronization/lifetime rules.
/// The result is borrowed, may be null, and acquires no reference or lock.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_canvas_get_video(canvas: *const c_void) -> *mut c_void {
    // SAFETY: the caller retains a readable live canvas.
    let fields = unsafe { obs_canvas_video_fields(canvas) };
    // SAFETY: C returns the address of that canvas's readable mix pointer.
    let mix = NonNull::new(unsafe { *fields.mix });
    let video = mix.map(|mix| {
        // SAFETY: a present mix is live; C borrows its readable video pointer field.
        unsafe { &*obs_canvas_video_output_field(mix.as_ptr()) }
    });
    crate::canvas_video_getter::borrowed_video(video)
}
