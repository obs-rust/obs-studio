//! Borrowed video pointer selection, preserving absent mix and NULL output.
/// Return the stored borrowed pointer when the mix exists, without acquiring a reference.
pub fn borrowed_video<T>(mix_video: Option<&*mut T>) -> *mut T {
    mix_video.copied().unwrap_or(core::ptr::null_mut())
}
