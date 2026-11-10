//! Operations on the owned error slot while its caller holds the encoder lock.

/// Drop the previous error before allocating a replacement; None clears it.
pub fn replace<T>(
    slot: &mut Option<T>,
    message: Option<&[u8]>,
    duplicate: impl FnOnce(&[u8]) -> T,
) {
    *slot = None;
    *slot = message.map(duplicate);
}

/// Return an independent owned copy, preserving an absent message.
pub fn duplicate<T, U>(slot: Option<&T>, copy: impl FnOnce(&T) -> U) -> Option<U> {
    slot.map(copy)
}
