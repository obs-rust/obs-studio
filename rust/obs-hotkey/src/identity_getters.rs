//! Read-only stored identity fields from `libobs/obs-hotkey-identity-getters.c.h`.
/// Return the stored hotkey ID without interpreting it.
pub fn id(_value: &usize) -> usize {
    todo!()
}

/// Return the stored name handle without reading its bytes.
pub fn name<H: Copy>(_value: &H) -> H {
    todo!()
}
