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
    if !output {
        return Err(Rejection::Output);
    }
    if !conversion {
        return Err(Rejection::Conversion);
    }
    if flags & 4 != 0 {
        return Err(Rejection::Encoded);
    }
    if flags & media == 0 {
        return Err(Rejection::Media);
    }
    Ok(())
}

/// Registration prepends, including duplicates and null tokens.
pub fn insertion_index() -> usize {
    0
}

/// Only the first exact callback/parameter pair is erased.
pub fn removal_index<T: PartialEq>(entries: &[T], entry: &T) -> Option<usize> {
    entries.iter().position(|value| value == entry)
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
    conversion_guard(output, present, flags, media)?;
    write(read());
    Ok(())
}
