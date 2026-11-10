//! Borrowed context getters; no allocation, reference acquisition or mutation.

/// Borrow the name token from an initialized context.
pub fn name<T: Copy>(field: &T) -> T {
    todo!("service context name")
}

/// Borrow the settings token from an initialized context.
pub fn settings<T: Copy>(field: &T) -> T {
    todo!("service context settings")
}
