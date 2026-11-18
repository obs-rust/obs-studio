//! Read-only predicates over borrowed weak-source state.

/// The native strong count is biased: zero still represents one owner.
pub fn expired(owners: Option<i64>) -> bool {
    owners.is_none_or(|owners| owners < 0)
}

/// Compare identity independently of the strong count, including after expiry.
pub fn references<T: Eq>(stored: Option<T>, source: Option<T>) -> bool {
    match (stored, source) {
        (Some(stored), Some(source)) => stored == source,
        _ => false,
    }
}
