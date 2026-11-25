//! Borrowed source-state queries and the centered-texture store; callers retain
//! ownership and synchronization.

pub fn texcoords_centered(value: &bool) -> bool {
    *value
}

pub fn is_hidden(value: &bool) -> bool {
    *value
}

/// Store `centered` into the source's centered-texture flag (#5599).
pub fn set_texcoords_centered(value: &mut bool, centered: bool) {
    *value = centered;
}
