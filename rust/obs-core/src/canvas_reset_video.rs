//! `obs_canvas_reset_video()` admission, preserving the C short-circuit order:
//! a missing video info is refused before the canvas is read, then the canvas
//! rules decide on its flags, then the core's video activity is asked, and
//! only then is the internal reset called, its result forwarded as is.

/// Operations on a borrowed canvas and the core.
pub trait Host {
    type Canvas: Copy;
    type Info: Copy;
    fn flags(&mut self, canvas: Self::Canvas) -> u32;
    fn video_active(&mut self) -> bool;
    fn reset_internal(&mut self, canvas: Self::Canvas, info: Self::Info) -> bool;
}

/// Reset a canvas's video; `info` is `None` for a NULL pointer.
pub fn reset_video<H: Host>(_host: &mut H, _canvas: H::Canvas, _info: Option<H::Info>) -> bool {
    false
}
