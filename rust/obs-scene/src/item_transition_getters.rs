//! Stored show/hide transition reads with null-item defaults.
/// Select a stored transition, returning the default for a missing item.
pub fn transition<T: Copy + Default>(values: Option<(T, T)>, show: bool) -> T {
    read_transition(
        values.map(|(shown, hidden)| (move || shown, move || hidden)),
        show,
    )
}

/// Read only the selected field, preserving the C getter's read footprint.
pub fn read_transition<T: Default>(
    values: Option<(impl FnOnce() -> T, impl FnOnce() -> T)>,
    show: bool,
) -> T {
    values
        .map(|(shown, hidden)| if show { shown() } else { hidden() })
        .unwrap_or_default()
}

/// Select a stored duration, returning zero for a missing item.
pub fn duration(values: Option<(u32, u32)>, show: bool) -> u32 {
    transition(values, show)
}
