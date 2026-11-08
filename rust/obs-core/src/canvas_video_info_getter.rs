//! Lazy video-info selection, preserving the graphics short circuit.
/// Select a borrowed mix value only when global graphics and a canvas exist.
pub fn video_info<C, T>(
    graphics: bool,
    canvas: Option<C>,
    mix: impl FnOnce(C) -> Option<T>,
) -> Option<T> {
    graphics.then(|| canvas.and_then(mix)).flatten()
}
