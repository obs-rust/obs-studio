//! Lazy video-info selection, preserving the graphics short circuit.
/// Select a borrowed mix value only when global graphics exists.
pub fn video_info<T>(graphics: bool, mix: impl FnOnce() -> Option<T>) -> Option<T> {
    graphics.then(mix).flatten()
}
