//! Module initialization ordering, independent of private C layouts.

/// Live module operations. OBS serializes module loading on its caller thread.
pub trait Lifecycle {
    /// Borrowed profiler identity.
    type Profile;
    fn loaded(&self) -> bool;
    fn set_loaded(&mut self, loaded: bool);
    fn profile_start(&mut self) -> Self::Profile;
    fn loading(&mut self, active: bool);
    fn load(&mut self) -> bool;
    fn warning(&mut self);
    fn profile_end(&mut self, profile: Self::Profile);
}

/// Initialize once successfully; a failed callback can be retried.
pub fn initialize<H: Lifecycle>(_core_present: bool, _module: Option<&mut H>) -> bool {
    todo!()
}
