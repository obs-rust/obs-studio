//! Source-specific rules around the shared module registry search (#4639).
use obs_util::module_registry::{self, Registry};

pub const INVALID: i32 = 0;
pub const ENABLED: i32 = 1;
pub const MISSING: i32 = 2;

/// Return a borrowed module identity; the registry retains ownership.
pub fn get_module<R: Registry>(registry: &R, id: Option<&[u8]>) -> Option<R::Handle> {
    module_registry::find(registry, id)
}

/// NULL is invalid; scene and group are enabled independently of registration.
pub fn load_state<R: Registry>(registry: &R, id: Option<&[u8]>) -> i32 {
    let Some(id) = id else {
        return INVALID;
    };
    if id == b"scene" || id == b"group" {
        return ENABLED;
    }
    get_module(registry, Some(id)).map_or(MISSING, |module| registry.load_state(module))
}
