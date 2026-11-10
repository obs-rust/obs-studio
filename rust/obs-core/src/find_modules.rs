//! Live traversal of registered module search paths.

/// The host owns paths, discovery, and synchronous callbacks. Each access is
/// bounded by the call, so a callback may append paths without a borrowed list.
pub trait Paths {
    fn has_core(&self) -> bool;
    fn count(&self) -> usize;
    fn discover(&mut self, index: usize);
}

/// Visit every registered path in order, including callback-time appends.
pub fn find<P: Paths>(_paths: &mut P) {
    todo!("module discovery Rust swap")
}
