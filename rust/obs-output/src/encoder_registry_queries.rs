//! Read-only results of the existing encoder registry lookup.

/// Unknown IDs have historically reported audio (zero).
pub fn kind(_registered: Option<i32>) -> i32 {
    todo!()
}

/// Return every registered capability bit, or zero for an unknown ID.
pub fn caps(_registered: Option<u32>) -> u32 {
    todo!()
}
