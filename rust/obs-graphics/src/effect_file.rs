//! Effect-file cache traversal and creation. Handles belong to the boundary;
//! the core controls validation order, byte equality, I/O and ownership.

/// Original diagnostics, emitted without changing their C format strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Diagnostic {
    /// No current graphics context.
    Context,
    /// Null file argument.
    File,
    /// File could not be read.
    Read,
}

/// Operations on the current context and its linked effects.
pub trait Model {
    /// A non-null file name handle.
    type File: Copy;
    /// A live cached or newly constructed effect.
    type Effect: Copy;
    /// File contents owned by the caller until `free`.
    type Text: Copy;
    /// Whether a graphics context is current.
    fn context_present(&mut self) -> bool;
    /// Head of the context's cache.
    fn first(&mut self) -> Option<Self::Effect>;
    /// Next cache entry.
    fn next(&mut self, effect: Self::Effect) -> Option<Self::Effect>;
    /// File name bytes, excluding the terminator.
    fn file_path(&self, file: Self::File) -> &[u8];
    /// Cached path bytes, excluding the terminator.
    fn effect_path(&self, effect: Self::Effect) -> &[u8];
    /// Load through the existing UTF-8/BOM reader.
    fn read(&mut self, file: Self::File) -> Option<Self::Text>;
    /// Forward the original constructor arguments and error-string storage.
    fn create(&mut self, text: Self::Text, file: Self::File) -> Option<Self::Effect>;
    /// Release the read buffer, including after constructor failure.
    fn free(&mut self, text: Self::Text);
    /// Emit one original diagnostic.
    fn log(&mut self, diagnostic: Diagnostic, file: Option<Self::File>);
}

/// First exact path match, in list order.
pub fn find_cached_effect<M: Model>(_model: &mut M, _file: M::File) -> Option<M::Effect> {
    todo!()
}

/// `gs_effect_create_from_file`, preserving its early-return and free order.
pub fn create_from_file<M: Model>(_model: &mut M, _file: Option<M::File>) -> Option<M::Effect> {
    todo!()
}
