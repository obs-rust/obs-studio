//! The graphics thread's stop check (`stop_requested()`), over the mix list.
//!
//! `obs_graphics_thread_loop()` asks it once per frame: the thread stops when
//! every mix's video output has stopped. The C takes `mixes_mutex` once around
//! the whole walk, reads `mixes.num` once under it, and asks every mix in
//! order, with no early exit once one is running; this keeps all three.

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
pub fn stop_requested(mixes: &mut impl Mixes) -> bool {
    let mut success = true;
    mixes.lock();
    for index in 0..mixes.count() {
        let video = mixes.video(index);
        if !mixes.stopped(video) {
            success = false;
        }
    }
    mixes.unlock();
    success
}
