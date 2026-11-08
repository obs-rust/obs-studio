//! Read the stored frame interval without conversion or clock access.

/// Return the stored nanosecond interval; an absent core has no interval.
pub fn interval(_value: Option<&u64>) -> u64 {
    todo!()
}
