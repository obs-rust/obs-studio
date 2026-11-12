//! Output conversion guard ordering and packet registration decisions.

/// The first rejected precondition, in C evaluation order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejection {
    Output,
    Conversion,
    Encoded,
    Media,
}

/// Validate a conversion without inspecting its contents.
pub fn conversion_guard(
    output: bool,
    conversion: bool,
    flags: u32,
    media: u32,
) -> Result<(), Rejection> {
    let _ = (output, conversion, flags, media);
    todo!()
}

/// Registration prepends, including duplicates and null tokens.
pub fn insertion_index() -> usize {
    todo!()
}

/// Only the first exact callback/parameter pair is erased.
pub fn removal_index<T: PartialEq>(entries: &[T], entry: &T) -> Option<usize> {
    let _ = (entries, entry);
    todo!()
}

/// Validate before reading the conversion or mutating the destination.
pub fn store<T>(
    output: bool,
    present: bool,
    flags: u32,
    media: u32,
    read: impl FnOnce() -> T,
    write: impl FnOnce(T),
) -> Result<(), Rejection> {
    let _ = (output, present, flags, media, read, write);
    todo!()
}
