//! Canvas channel lookup delegates reference acquisition and locking to its view.
/// Forward a present canvas and exact channel; the host returns a new source reference.
pub fn get_channel<C, S>(
    canvas: Option<C>,
    channel: u32,
    get_source: impl FnOnce(C, u32) -> Option<S>,
) -> Option<S> {
    canvas.and_then(|canvas| get_source(canvas, channel))
}
