//! Source type defaults: a new settings object filled by one callback.

/// The defaults callback a source type registered; `get_defaults2` wins
/// when both are set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Callback {
    /// `get_defaults2(type_data, settings)`.
    Modern,
    /// `get_defaults(settings)`.
    Legacy,
    /// Neither: the settings object stays empty.
    Neither,
}

/// Picks the callback from which of the two the type has.
pub fn callback(modern: bool, legacy: bool) -> Callback {
    let _ = (modern, legacy);
    todo!("#5640")
}

/// `obs_get_source_defaults()` once the type is looked up: an unknown type
/// (`None`) creates nothing; a known one creates a settings object, runs its
/// callback on it (none for [`Callback::Neither`]) and hands it back.
pub fn defaults<S>(
    found: Option<Callback>,
    create: impl FnOnce() -> S,
    fill: impl FnOnce(Callback, &S),
) -> Option<S> {
    let _ = (found, create, fill);
    todo!("#5640")
}
