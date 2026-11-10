//! A thread's call stack (#4525): the return addresses on another thread of
//! this process, innermost first, captured without allocating.

/// A thread whose stack can be captured later from another thread.
pub struct Thread(crate::platform::Thread);

/// Whether this host can capture another thread's stack ([`capture`] gives
/// no frames if not).
pub const SUPPORTED: bool = crate::platform::STACK_SUPPORTED;

/// The calling thread.
pub fn current() -> Thread {
    Thread(crate::platform::current_thread())
}

/// Loads what a capture needs (call once before, outside a signal).
pub fn prepare() {
    crate::platform::prepare_stack();
}

/// Captures `thread`'s return addresses into `out`, innermost first; returns
/// how many. Must not be called on `thread` itself.
pub fn capture(thread: &Thread, out: &mut [usize]) -> usize {
    crate::platform::capture_stack(&thread.0, out)
}
