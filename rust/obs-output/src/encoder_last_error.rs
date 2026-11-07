//! Borrow the stored error unchanged; only an absent encoder emits a diagnostic.

pub fn message<T: Default>(field: Option<T>, invalid_encoder: impl FnOnce()) -> T {
    let _ = (field, invalid_encoder);
    todo!("#3900")
}
