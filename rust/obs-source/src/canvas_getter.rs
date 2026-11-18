//! Acquire an independently owned canvas from a source's borrowed weak token.
/// Forward the stored token once; the existing acquisition operation owns
/// null/expired handling and the strong-reference increment.
pub fn get<W, R>(weak: W, acquire: impl FnOnce(W) -> R) -> R {
    acquire(weak)
}
