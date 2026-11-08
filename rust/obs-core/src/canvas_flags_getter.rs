//! Stored canvas flags and removal status without filtering or mutation.

/// Return every stored bit, including private and unknown bits.
pub fn stored_flags(flags: &u32) -> u32 {
    *flags
}

/// Whether the stored flags mark the canvas as removed.
pub fn stored_removed(flags: &u32) -> bool {
    crate::canvas_rules::removed(*flags)
}
