//! User visibility selection after the C boundary atomically reads the item.

/// Return false for an absent item, otherwise its atomically read user visibility.
pub fn visible(user_visible: Option<bool>) -> bool {
    user_visible.unwrap_or(false)
}
