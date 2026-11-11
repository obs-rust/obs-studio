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
    let field = match effect {
        0 => Field::Default,
        1 => Field::DefaultRect,
        2 => Field::Opaque,
        3 => Field::Solid,
        4 => Field::Bicubic,
        5 => Field::Lanczos,
        6 => Field::BilinearLowres,
        7 => Field::PremultipliedAlpha,
        8 => Field::Repeat,
        9 => Field::Area,
        _ => return T::default(),
    };
    read(field)
}
