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
    let mut owners = item.owners();
    while owners > 0 {
        match item.compare_owners(owners, owners.wrapping_add(1)) {
            Ok(()) => return true,
            Err(observed) => owners = observed,
        }
    }
    false
}

/// Run a show/hide callback, releasing before returning its handled result.
pub fn trigger(item: &mut impl Item, show: bool, pressed: bool) -> bool {
    let acquired = acquire(item);
    let handled = pressed && acquired && item.visible() != show;
    if handled {
        item.set_visible(show);
    }
    item.release(acquired);
    handled
}

/// Replace every `%1` byte sequence; an initially empty template stays NULL.
pub fn description(template: &[u8], name: &[u8]) -> Option<Vec<u8>> {
    let template = template.split(|&b| b == 0).next().unwrap_or_default();
    let name = name.split(|&b| b == 0).next().unwrap_or_default();
    if template.is_empty() {
        return None;
    }
    let mut result = Vec::new();
    let mut rest = template;
    while let Some(pos) = rest.windows(2).position(|bytes| bytes == b"%1") {
        result.extend_from_slice(&rest[..pos]);
        result.extend_from_slice(name);
        rest = &rest[pos + 2..];
    }
    result.extend_from_slice(rest);
    Some(result)
}
