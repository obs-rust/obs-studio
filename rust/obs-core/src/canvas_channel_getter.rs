//! Canvas channel lookup delegates reference acquisition and locking to its view.
/// Forward the exact view and channel; the host returns a new source reference.
pub fn get_channel<V, S>(view: V, channel: u32, get_source: impl FnOnce(V, u32) -> S) -> S {
    get_source(view, channel)
}
