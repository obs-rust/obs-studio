//! Borrowed source selection for scenes and their items.

/// Return the stored source token, or the null/default token for an absent handle.
pub fn source<T: Default>(stored: Option<T>) -> T {
    stored.unwrap_or_default()
}
