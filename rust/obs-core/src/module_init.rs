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
pub fn initialize<H: Lifecycle>(core_present: bool, module: Option<&mut H>) -> bool {
    if !core_present {
        return false;
    }
    let Some(module) = module else {
        return false;
    };
    if module.loaded() {
        return true;
    }
    let profile = module.profile_start();
    module.loading(true);
    let loaded = module.load();
    module.set_loaded(loaded);
    module.loading(false);
    if !module.loaded() {
        module.warning();
    }
    module.profile_end(profile);
    // Diagnostics and profiler callbacks can observe or change module state.
    module.loaded()
}
