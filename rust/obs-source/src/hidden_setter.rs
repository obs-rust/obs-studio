//! Hidden-state assignment; ownership and synchronization stay with the caller.

pub fn set_hidden(current: &mut bool, hidden: bool) {
    *current = hidden;
}
