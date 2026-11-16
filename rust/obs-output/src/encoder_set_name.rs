//! Encoder names are not uniquified; ignore absent, empty and byte-equal names.
/// Read the current name only when a nonempty name was proposed, as C does.
pub fn should_rename<'a>(current: impl FnOnce() -> &'a [u8], name: Option<&[u8]>) -> bool {
    name.is_some_and(|name| !name.is_empty() && name != current())
}
