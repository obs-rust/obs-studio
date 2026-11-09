//! Encoder creation ordering. The boundary owns context, mutexes and callbacks.

/// Operations on a live, caller-owned encoder. Indices 0..6 denote init,
/// callbacks, outputs, pause, ROI and last-error mutexes, respectively.
pub trait Boundary {
    /// Put a mutex into its cleanup-safe uninitialized state.
    fn sentinel(&mut self, index: u32);
    /// Initialize the context with the caller's name, settings and hotkeys.
    fn context(&mut self) -> bool;
    /// Initialize one mutex, preserving its recursive attribute.
    fn mutex(&mut self, index: u32, recursive: bool) -> bool;
    /// Dispatch the optional legacy defaults callback.
    fn defaults(&mut self);
    /// Dispatch the current optional defaults2 callback and type data.
    fn defaults2(&mut self);
}

/// Initialize without rolling back: the caller owns partial-state cleanup.
pub fn initialize(boundary: &mut impl Boundary) -> bool {
    for index in 0..6 {
        boundary.sentinel(index);
    }
    if !boundary.context() {
        return false;
    }
    for index in 0..6 {
        if !boundary.mutex(index, index < 2) {
            return false;
        }
    }
    boundary.defaults();
    boundary.defaults2();
    true
}
