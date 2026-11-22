//! Scene-item reference acquisition over a C-owned atomic counter. C keeps
//! the counter's layout, its platform `long` width and the atomic ordering.

/// The one native operation on a live item, without borrowing its layout.
pub trait ItemRef {
    /// Atomically add one to the item's stored reference count.
    fn increment(&self);
}

/// A missing item is a silent no-op; a present one gets exactly one atomic
/// increment and nothing else. Zero, negative and wrapped counts are not
/// inspected (characterized, not endorsed).
pub fn addref(item: Option<&impl ItemRef>) {
    let _ = item;
    todo!()
}
