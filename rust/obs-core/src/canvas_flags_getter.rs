//! Stored canvas flags without filtering or mutation.

/// Return every stored bit, including private and unknown bits.
pub fn stored_flags(flags: &u32) -> u32 {
    *flags
}
