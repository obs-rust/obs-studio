//! Raw video callbacks delegate to libobs' own start/stop on the main
//! canvas's mix video; the divisor and duplicate checks, the registration
//! list, its locking and the `raw_active` count stay there.

/// The warning `obs_add_raw_video_callback2` logs when the main canvas has
/// no mix yet, word for word (also when reached through the divisor-1 form).
pub const NOT_SET_UP: &core::ffi::CStr =
    c"obs_add_raw_video_callback2: video is not set up yet, the raw video callback was not added";

/// Access to the main canvas's mix and libobs' start/stop.
pub trait Video {
    type Video;
    type Conversion;
    type Callback;
    type Param;

    /// The main canvas's mix's video, `None` when there is no main canvas
    /// or no mix (before the first video reset). A mix's video is passed on
    /// as read, NULL included.
    fn main_video(&mut self) -> Option<Self::Video>;
    /// Log [`NOT_SET_UP`] at `LOG_WARNING`.
    fn warn_not_set_up(&mut self);
    fn start(
        &mut self,
        video: Self::Video,
        conversion: Self::Conversion,
        frame_rate_divisor: u32,
        callback: Self::Callback,
        param: Self::Param,
    );
    fn stop(&mut self, video: Self::Video, callback: Self::Callback, param: Self::Param);
}

/// `obs_add_raw_video_callback`: the divisor-1 form of [`add2`].
pub fn add<V: Video>(
    video: &mut V,
    conversion: V::Conversion,
    callback: V::Callback,
    param: V::Param,
) {
    let _ = (video, conversion, callback, param);
    todo!("obs_add_raw_video_callback")
}

/// `obs_add_raw_video_callback2`: one start on the main mix's video with the
/// arguments unchanged, or the warning and nothing kept when there is no mix.
pub fn add2<V: Video>(
    video: &mut V,
    conversion: V::Conversion,
    frame_rate_divisor: u32,
    callback: V::Callback,
    param: V::Param,
) {
    let _ = (video, conversion, frame_rate_divisor, callback, param);
    todo!("obs_add_raw_video_callback2")
}

/// `obs_remove_raw_video_callback`: one stop on the main mix's video, or
/// nothing when there is no mix.
pub fn remove<V: Video>(video: &mut V, callback: V::Callback, param: V::Param) {
    let _ = (video, callback, param);
    todo!("obs_remove_raw_video_callback")
}
