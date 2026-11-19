//! Validate a borrowed source before its synchronous property-refresh notification.

/// C-owned validation and source signal boundary.
pub trait Source {
    /// Validate the source, reporting the original diagnostic if it is absent.
    fn valid(&self) -> bool;
    /// Emit the source notification synchronously, with no core notification.
    fn signal_update_properties(&self);
}

pub fn update_properties(_source: &impl Source) {
    todo!("port obs_source_update_properties")
}
