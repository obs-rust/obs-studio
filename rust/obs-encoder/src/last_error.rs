//! Operations on the owned error slot while its caller holds the encoder lock.

/// Drop the previous error before allocating a replacement; None clears it.
pub fn replace<T>(
    slot: &mut Option<T>,
    message: Option<&[u8]>,
    duplicate: impl FnOnce(&[u8]) -> T,
) {
    let _ = (slot, message, duplicate);
    todo!()
}

/// Return an independent owned copy, preserving an absent message.
pub fn duplicate<T, U>(slot: Option<&T>, copy: impl FnOnce(&T) -> U) -> Option<U> {
    let _ = (slot, copy);
    todo!()
}
