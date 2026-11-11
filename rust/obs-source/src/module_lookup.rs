//! Source-specific rules around the shared module registry search (#4639).
use obs_util::module_registry::Registry;

pub const INVALID: i32 = 0;
pub const ENABLED: i32 = 1;
pub const MISSING: i32 = 2;

/// Return a borrowed module identity; the registry retains ownership.
pub fn get_module<R: Registry>(_registry: &R, _id: Option<&[u8]>) -> Option<R::Handle> {
    todo!("#4639 source module lookup")
}

/// NULL is invalid; scene and group are enabled independently of registration.
pub fn load_state<R: Registry>(_registry: &R, _id: Option<&[u8]>) -> i32 {
    todo!("#4639 source module load state")
}
