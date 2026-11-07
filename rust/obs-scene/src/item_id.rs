//! Read the stored scene item ID, preserving the null-item diagnostic.

/// A missing item logs once and returns zero; a live item's signed ID is exact.
pub fn get_id(value: Option<i64>, log_null: impl FnOnce()) -> i64 {
    match value {
        Some(id) => id,
        None => {
            log_null();
            0
        }
    }
}
