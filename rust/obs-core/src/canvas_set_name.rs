//! Canvas rename orchestration. C retains the opaque canvas and context layouts.
//! Names are bytes; removed canvases deliberately remain renameable.

/// Operations on a live canvas and the requested name, captured by the host.
pub trait Host {
    /// A copy of the old name, allocated before updating the context.
    type Previous;
    /// Signal payload, retained until both notifications have returned.
    type Data;
    fn requested(&self) -> Option<&[u8]>;
    fn flags(&self) -> u32;
    fn current(&self) -> &[u8];
    fn private(&self) -> bool;
    fn save_previous(&mut self) -> Self::Previous;
    fn rename_private(&mut self);
    fn rename_public(&mut self);
    fn data(&mut self, previous: &Self::Previous) -> Self::Data;
    fn signal_local(&mut self, data: &mut Self::Data);
    fn signal_global(&mut self, data: &mut Self::Data);
    fn free_data(&mut self, data: Self::Data);
    fn free_previous(&mut self, previous: Self::Previous);
}

/// Run the original setter's short circuits, update, notifications and cleanup.
pub fn set_name(host: &mut impl Host) {
    if crate::canvas_rules::name_empty(host.requested().and_then(|name| name.first().copied())) {
        return;
    }
    if !crate::canvas_rules::renames(host.flags()) {
        return;
    }
    if host.requested() == Some(host.current()) {
        return;
    }
    let previous = host.save_previous();
    if host.private() {
        host.rename_private();
    } else {
        host.rename_public();
    }
    let mut data = host.data(&previous);
    host.signal_local(&mut data);
    // Re-read after callbacks: the C setter does not snapshot private state.
    if !host.private() {
        host.signal_global(&mut data);
    }
    host.free_data(data);
    host.free_previous(previous);
}
