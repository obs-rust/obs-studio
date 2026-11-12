//! Output conversion guard ordering and packet registration decisions, from
//! `libobs/obs-output-conversion-setters.c.h` and
//! `libobs/obs-output-packet-callbacks.c.h`.
//!
//! The checks run in the C order: the output pointer, the conversion
//! pointer, the encoded flag, then the media flag, and only the first
//! failing one is reported. A conversion is read and stored only after all
//! of them pass, the struct before its `_set` flag.

/// `OBS_OUTPUT_VIDEO`, the `media` of the video conversion setter.
pub const VIDEO: u32 = 1 << 0;
/// `OBS_OUTPUT_AUDIO`, the `media` of the audio conversion setter.
pub const AUDIO: u32 = 1 << 1;
/// `OBS_OUTPUT_ENCODED`: an encoded output takes no conversion.
pub const ENCODED: u32 = 1 << 2;

/// The first rejected precondition, in C evaluation order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejection {
    Output,
    Conversion,
    Encoded,
    Media,
}

/// Validate a conversion without inspecting its contents. `media` is
/// [`VIDEO`] or [`AUDIO`].
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
    if flags & ENCODED != 0 {
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
