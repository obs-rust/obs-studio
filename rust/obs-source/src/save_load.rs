//! Save/load ordering over a live source backend. Callbacks may change source fields.

/// Borrowed access to the existing source and its direct filters.
pub trait Backend {
    /// Source handle; ownership remains with the caller.
    type Source: Copy;
    /// Validate the handle and plugin data, preserving entry-point diagnostics.
    fn valid(&mut self, source: Self::Source, name: &'static str) -> bool;
    /// Emit the existing global/local source signals.
    fn signal(&mut self, source: Self::Source, save: bool);
    /// Invoke the optional callback with the current plugin data and settings.
    fn callback(&mut self, source: Self::Source, save: bool);
    /// Current number of directly attached filters.
    fn filter_count(&mut self, source: Self::Source) -> usize;
    /// Current filter at a stored index.
    fn filter(&mut self, source: Self::Source, index: usize) -> Self::Source;
}

/// Save signals precede the optional callback.
pub fn save<B: Backend>(_backend: &mut B, _source: B::Source) {
    todo!()
}

/// The optional load callback precedes load signals.
pub fn load<B: Backend>(_backend: &mut B, _source: B::Source) {
    todo!()
}

/// Load the parent, then direct filters in reverse stored order.
pub fn load2<B: Backend>(_backend: &mut B, _source: B::Source) {
    todo!()
}
