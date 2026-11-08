//! Stored show/hide transition reads with null-item defaults.
/// Select a stored transition, returning the default for a missing item.
pub fn transition<T: Copy + Default>(values: Option<(T, T)>, show: bool) -> T {
    let _ = (values, show);
    todo!("port transition pointer selection")
}

/// Select a stored duration, returning zero for a missing item.
pub fn duration(values: Option<(u32, u32)>, show: bool) -> u32 {
    let _ = (values, show);
    todo!("port transition duration selection")
}
