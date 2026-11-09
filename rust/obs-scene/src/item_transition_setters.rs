//! Stored transition replacement, preserving release-before-acquire ordering.

/// Access to one item, with reference operations supplied by its owner.
pub trait Item {
    type Source: Copy;
    fn transition(&self, show: bool) -> Option<Self::Source>;
    fn release(&mut self, source: Self::Source);
    fn get_ref(&mut self, source: Option<Self::Source>) -> Option<Self::Source>;
    fn store_transition(&mut self, show: bool, source: Option<Self::Source>);
    fn store_duration(&mut self, show: bool, duration: u32);
}

/// Replace only the selected transition; absent items do nothing.
pub fn set_transition<I: Item>(item: Option<&mut I>, show: bool, source: Option<I::Source>) {
    let _ = (item, show, source);
    todo!()
}

/// Store the complete unsigned duration; absent items do nothing.
pub fn set_duration<I: Item>(item: Option<&mut I>, show: bool, duration: u32) {
    let _ = (item, show, duration);
    todo!()
}
