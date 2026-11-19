//! Lazy selection for the core video-info query.
/// Return the stored snapshot only after the core and graphics gates pass.
pub fn video_info<C, T>(
    core: Option<C>,
    graphics: impl FnOnce(&C) -> bool,
    mix: impl FnOnce(C) -> Option<T>,
) -> Option<T> {
    core.filter(graphics).and_then(mix)
}
