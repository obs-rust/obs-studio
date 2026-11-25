//! Borrowed source-state queries; callers retain ownership and synchronization.

pub fn texcoords_centered(value: &bool) -> bool {
    *value
}

pub fn is_hidden(value: &bool) -> bool {
    *value
}

/// Store `centered` into the source's centered-texture flag (#5599).
#[expect(clippy::todo, reason = "RED stub for #5599")]
pub fn set_texcoords_centered(_value: &mut bool, _centered: bool) {
    todo!("#5599")
}
