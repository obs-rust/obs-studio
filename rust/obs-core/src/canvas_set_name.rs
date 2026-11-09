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
pub fn set_name(_host: &mut impl Host) {
    todo!()
}
