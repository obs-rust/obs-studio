//! Read-only predicates over borrowed weak-source state.

/// The native strong count is biased: zero still represents one owner.
pub fn expired(_owners: Option<i64>) -> bool {
    todo!("port obs_weak_source_expired")
}

/// Compare identity independently of the strong count, including after expiry.
pub fn references<T: Eq>(_stored: Option<T>, _source: Option<T>) -> bool {
    todo!("port obs_weak_source_references_source")
}
