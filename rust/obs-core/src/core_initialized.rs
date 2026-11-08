//! Read the core pointer's presence without dereferencing it.

/// Whether the global core pointer has been set by startup.
pub fn initialized<T>(core: *const T) -> bool {
    !core.is_null()
}
