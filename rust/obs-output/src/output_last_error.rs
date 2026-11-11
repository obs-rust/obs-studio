//! Replace an owned output error, dropping the old allocation before duplicating.

/// `None` clears the message; an empty message is still an owned value.
pub fn replace<T>(
    slot: &mut Option<T>,
    message: Option<&[u8]>,
    duplicate: impl FnOnce(&[u8]) -> T,
) {
    *slot = None;
    *slot = message.map(duplicate);
}
