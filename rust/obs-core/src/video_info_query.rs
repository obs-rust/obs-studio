//! Lazy selection for the core video-info query.
/// Return the stored snapshot only after the core and graphics gates pass.
pub fn video_info<C, T>(
    _core: Option<C>,
    _graphics: impl FnOnce(&C) -> bool,
    _mix: impl FnOnce(C) -> Option<T>,
) -> Option<T> {
    todo!("port obs_get_video_info")
}
