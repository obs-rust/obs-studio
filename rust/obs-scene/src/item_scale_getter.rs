//! Scale reads choose the stored value or the existing C canvas conversion.

/// A missing item leaves output untouched. Relative values use the callback
/// while the caller holds the captured parent's video mutex.
pub fn read_scale(
    value: Option<([u32; 2], bool)>,
    relative: impl FnOnce() -> [u32; 2],
    output: &mut [u32; 2],
) {
    let _ = (value, relative, output);
    todo!("#3897 scale getter")
}
