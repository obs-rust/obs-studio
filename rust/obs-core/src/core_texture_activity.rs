//! Read-only texture conversion flags, absent until a video mix exists.

/// Return both stored flags, or false for an absent mix.
pub fn activity(flags: Option<(&bool, &bool)>) -> (bool, bool) {
    flags.map_or((false, false), |(nv12, p010)| (*nv12, *p010))
}
