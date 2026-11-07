//! Read the stored scene item ID, preserving the null-item diagnostic.

/// A missing item logs once and returns zero; a live item's signed ID is exact.
pub fn get_id(_value: Option<i64>, _log_null: impl FnOnce()) -> i64 {
    todo!()
}
