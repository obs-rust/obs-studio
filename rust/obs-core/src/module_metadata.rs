//! Borrowed module metadata selection; no allocation or string decoding.

/// Read a field, returning its null/default value for an absent module.
pub fn field<T>(value: Option<T>, null: T) -> T {
    value.unwrap_or(null)
}

/// Prefer a non-null metadata display name, otherwise invoke the callback.
pub fn name<T>(display: Option<T>, callback: Option<impl FnOnce() -> T>, null: T) -> T {
    display.unwrap_or_else(|| callback.map_or_else(|| null, |call| call()))
}
