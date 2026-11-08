//! Read the core pointer's presence without dereferencing it.

/// Whether the global core pointer has been set by startup.
pub fn initialized<T>(_core: *const T) -> bool {
    todo!("port obs_initialized")
}
