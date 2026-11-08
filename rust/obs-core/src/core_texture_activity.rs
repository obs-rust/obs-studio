//! Read-only texture conversion flags, absent until a video mix exists.

/// Return both stored flags, or false for an absent mix.
pub fn activity(_flags: Option<(&bool, &bool)>) -> (bool, bool) {
    todo!("port core texture activity queries")
}
