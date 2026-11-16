//! Read and replace the stored scene item ID, preserving C behavior.

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

/// Replace the stored signed ID exactly, without validation or normalization.
pub fn set_id(stored: &mut i64, id: i64) {
    *stored = id;
}
