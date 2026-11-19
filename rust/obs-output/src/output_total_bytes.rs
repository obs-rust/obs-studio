//! Output total-bytes query, independent of the private output layout.

/// One live output seen through its borrowed callback, delay flags and data.
pub trait Host {
    /// Whether the current `info.get_total_bytes` is set (a fresh read).
    fn has_callback(&self) -> bool;
    /// The atomic `delay_active` flag.
    fn delay_active(&self) -> bool;
    /// The atomic `delay_capturing` flag.
    fn delay_capturing(&self) -> bool;
    /// Calls the current callback once with the current `context.data`.
    fn dispatch(&self) -> u64;
}

/// Preserve the C order: NULL diagnostic, callback presence before the delay
/// loads, short-circuit delay reads, then exactly one dispatch.
pub fn query<H: Host>(_host: Option<&H>, _null_output: impl FnOnce()) -> u64 {
    0
}
