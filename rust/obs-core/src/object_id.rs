//! Lazy borrowed-ID selection without projecting unsupported objects.

/// Concrete object kinds whose private layouts contain an info ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Source,
    Output,
    Encoder,
    Service,
}

/// Read only the ID selected by a supported kind. Missing objects and all
/// other kinds leave the projection closure untouched.
pub fn query<T>(_kind: Option<i32>, _read: impl FnOnce(Kind) -> T) -> Option<T> {
    todo!("generic object ID swap")
}
