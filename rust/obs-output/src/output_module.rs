//! Output module queries over the shared borrowed registry (#4654).
use obs_util::module_registry::{self, Query, Registry};

pub const MISSING: i32 = 2;

/// Return a borrowed identity without changing module storage.
pub fn get_module<R: Registry>(registry: &R, id: impl Query) -> Option<R::Handle> {
    module_registry::find(registry, id)
}

/// Return the stored state, or MISSING when no output ID matches.
pub fn load_state<R: Registry>(registry: &R, id: impl Query) -> i32 {
    get_module(registry, id).map_or(MISSING, |module| registry.load_state(module))
}
