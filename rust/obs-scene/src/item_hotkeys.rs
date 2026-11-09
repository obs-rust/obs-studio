//! Scene-item hotkey reference acquisition, event ordering and byte descriptions.

/// The private item operations; implementations keep the callback's item alive.
pub trait Item {
    fn owners(&mut self) -> i64;
    fn compare_owners(&mut self, expected: i64, next: i64) -> Result<(), i64>;
    fn visible(&mut self) -> bool;
    fn set_visible(&mut self, visible: bool);
    fn release(&mut self, acquired: bool);
}

/// Acquire only a positive reference count, retrying with the CAS observation.
pub fn acquire(item: &mut impl Item) -> bool {
    todo!()
}

/// Run a show/hide callback, releasing before returning its handled result.
pub fn trigger(item: &mut impl Item, show: bool, pressed: bool) -> bool {
    todo!()
}

/// Replace every `%1` byte sequence; an initially empty template stays NULL.
pub fn description(template: &[u8], name: &[u8]) -> Option<Vec<u8>> {
    todo!()
}
