//! Lazy video-info selection, preserving the graphics short circuit.
/// Select a borrowed mix value only when global graphics exists.
pub fn video_info<T>(_graphics: bool, _mix: impl FnOnce() -> Option<T>) -> Option<T> {
    todo!("#3950 canvas video info")
}
