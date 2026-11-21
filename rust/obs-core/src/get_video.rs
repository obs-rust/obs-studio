//! Borrow the main canvas's current video output without changing ownership.
/// The main canvas's mix's video, or `None` when there is no main canvas, no
/// mix yet (before the first video reset) or no video in the mix. Each step
/// is one borrowed read: no reference is acquired and nothing is written.
pub fn get_video<C, M, V>(
    canvas: Option<C>,
    mix: impl FnOnce(C) -> Option<M>,
    video: impl FnOnce(M) -> Option<V>,
) -> Option<V> {
    let _ = (canvas, mix, video);
    todo!()
}
