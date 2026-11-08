//! Stored canvas flags without filtering or mutation.

/// Return every stored bit, including private and unknown bits.
pub fn stored_flags(flags: &u32) -> u32 {
    *flags
}

/// Whether the stored flags mark the canvas as removed.
pub fn stored_removed(_flags: &u32) -> bool {
    todo!("port obs_canvas_removed")
}
