//! The graphics thread's stop check (`stop_requested()`), over the mix list.

/// The mix list as the stop check sees it: `mixes_mutex`, `mixes.num`, mix
/// `i`'s `video` and `video_output_stopped()`.
pub trait Mixes {
    /// A mix's `video_t *`, passed through unread.
    type Video: Copy;
    fn lock(&mut self);
    fn unlock(&mut self);
    fn count(&mut self) -> usize;
    fn video(&mut self, index: usize) -> Self::Video;
    fn stopped(&mut self, video: Self::Video) -> bool;
}

/// True when every mix's output has stopped; an empty list is stopped.
pub fn stop_requested(_mixes: &mut impl Mixes) -> bool {
    todo!()
}
