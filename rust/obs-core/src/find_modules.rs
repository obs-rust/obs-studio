//! Live traversal of registered module search paths.

/// The host owns paths, discovery, and synchronous callbacks. Each access is
/// bounded by the call, so a callback may append paths without a borrowed list.
pub trait Paths {
    fn has_core(&self) -> bool;
    fn count(&self) -> usize;
    fn discover(&mut self, index: usize);
}

/// Visit every registered path in order, including callback-time appends.
pub fn find<P: Paths>(paths: &mut P) {
    if !paths.has_core() {
        return;
    }
    let mut index = 0;
    while index < paths.count() {
        paths.discover(index);
        index += 1;
    }
}
