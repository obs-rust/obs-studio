//! Encoder module queries over the shared borrowed registry (#4659).
use obs_util::module_registry::{Query, Registry};

pub const MISSING: i32 = 2;

/// Return a borrowed identity without changing module storage.
pub fn get_module<R: Registry>(_registry: &R, _id: impl Query) -> Option<R::Handle> {
    todo!("encoder module port")
}

/// Return the stored state, or MISSING when no encoder ID matches.
pub fn load_state<R: Registry>(_registry: &R, _id: impl Query) -> i32 {
    todo!("encoder module port")
}
