//! Borrowed, byte-exact lookup in the C-owned service registry.

/// Walk IDs lazily and return the first matching entry without copying it.
pub fn find<'a, T>(id: &[u8], entries: impl IntoIterator<Item = (&'a [u8], T)>) -> Option<T> {
    let _ = (id, entries);
    todo!("port find_service")
}
