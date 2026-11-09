//! Read-only results of the existing encoder registry lookup.

/// Unknown IDs have historically reported audio (zero).
pub fn kind(registered: Option<i32>) -> i32 {
    registered.unwrap_or(0)
}

/// Return every registered capability bit, or zero for an unknown ID.
pub fn caps(registered: Option<u32>) -> u32 {
    registered.unwrap_or(0)
}
