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
    let _ = item;
    todo!()
}

/// Run a show/hide callback, releasing before returning its handled result.
pub fn trigger(item: &mut impl Item, show: bool, pressed: bool) -> bool {
    let _ = (item, show, pressed);
    todo!()
}

/// Replace every `%1` byte sequence; an initially empty template stays NULL.
pub fn description(template: &[u8], name: &[u8]) -> Option<Vec<u8>> {
    let _ = (template, name);
    todo!()
}

/// The rename handler's operations, asked for in the C's order.
pub trait Rename {
    /// `calldata_string(data, "new_name")`; `None` for NULL.
    fn new_name(&mut self) -> Option<Vec<u8>>;
    /// The current show or hide template of `obs->hotkeys`; `None` for NULL.
    fn template(&mut self, show: bool) -> Option<Vec<u8>>;
    /// The item's `toggle_visibility` pair.
    fn pair(&mut self) -> usize;
    /// `obs_hotkey_pair_set_descriptions()`, `None` for a NULL description.
    fn set_descriptions(&mut self, pair: usize, show: Option<&[u8]>, hide: Option<&[u8]>);
}

/// `sceneitem_renamed()`: the name, then each template with its
/// description built before the next read, then the pair and the setter.
pub fn rename(ops: &mut impl Rename) {
    let _ = ops;
    todo!()
}
