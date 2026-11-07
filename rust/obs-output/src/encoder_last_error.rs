//! Borrow the stored error unchanged; only an absent encoder emits a diagnostic.

/// `None` means no encoder; `Some` contains its stored message, including NULL.
pub fn message<T: Default>(field: Option<T>, invalid_encoder: impl FnOnce()) -> T {
    match field {
        Some(value) => value,
        None => {
            invalid_encoder();
            T::default()
        }
    }
}
