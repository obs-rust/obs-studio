//! Borrowed video pointer selection, preserving absent mix and NULL output.
/// Return the stored borrowed pointer when the mix exists.
pub fn borrowed_video<T>(mix_video: Option<&*mut T>) -> *mut T {
    todo!("#3945: {mix_video:?}")
}
