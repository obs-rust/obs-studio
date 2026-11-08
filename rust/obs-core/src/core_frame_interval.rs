//! Read the stored frame interval without conversion or clock access.

/// Return the stored nanosecond interval; an absent core has no interval.
pub fn interval(value: Option<&u64>) -> u64 {
    value.copied().unwrap_or_default()
}
