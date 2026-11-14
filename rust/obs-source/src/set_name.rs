//! Source rename orchestration, `obs_source_set_name()`. C keeps the source
//! and context layouts, the name tables and the canvas rename; Rust decides
//! between no change, the canvas path and the table path, and whether the
//! global signal goes out. Names are bytes.

/// Operations on a live source and the requested name, captured by the host.
pub trait Source {
    /// A copy of the old name, taken before the context is renamed.
    type Previous;
    /// Signal payload, kept until both notifications have returned.
    type Data;
    /// The requested name, `None` for NULL.
    fn requested(&self) -> Option<&[u8]>;
    /// The context's current name, `None` for NULL.
    fn current(&self) -> Option<&[u8]>;
    /// Whether the source's output flags hold `OBS_SOURCE_REQUIRES_CANVAS`.
    fn requires_canvas(&self) -> bool;
    fn private(&self) -> bool;
    /// `obs_canvas_rename_source()`, which renames and signals itself.
    fn rename_canvas(&mut self);
    fn save_previous(&mut self) -> Self::Previous;
    /// The context setter on the public source table.
    fn rename_public(&mut self);
    /// The context setter with no table.
    fn rename_private(&mut self);
    /// The payload: the source, the context's new name and the previous one.
    fn data(&mut self, previous: &Self::Previous) -> Self::Data;
    /// `source_rename` on the core signal handler.
    fn signal_global(&mut self, data: &mut Self::Data);
    /// `rename` on the source's own signal handler.
    fn signal_local(&mut self, data: &mut Self::Data);
    fn free_data(&mut self, data: Self::Data);
    fn free_previous(&mut self, previous: Self::Previous);
}

/// Run the original setter: the NULL check, the unchanged-name check, then
/// the canvas rename or the table rename with its signals and cleanup.
pub fn set_name<S: Source>(source: Option<&mut S>, log_null: impl FnOnce()) {
    let _ = (source, log_null);
    todo!("obs_source_set_name")
}
