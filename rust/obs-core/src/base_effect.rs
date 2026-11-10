//! Select a borrowed video effect without reading storage for unknown IDs.

/// Private video field selected by the public base-effect enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Default,
    DefaultRect,
    Opaque,
    Solid,
    Bicubic,
    Lanczos,
    BilinearLowres,
    PremultipliedAlpha,
    Repeat,
    Area,
}

/// Read exactly the selected field, or return the empty value for an unknown ID.
pub fn get<T: Default>(effect: i32, read: impl FnOnce(Field) -> T) -> T {
    todo!("#4567: select the borrowed base effect")
}
