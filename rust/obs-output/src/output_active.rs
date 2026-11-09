//! Output active query independent of the private C layout.

/// Preserve the silent NULL default and lazy reconnecting query.
pub fn get(active: Option<bool>, reconnecting: impl FnOnce() -> bool) -> bool {
    active.is_some_and(|active| active || reconnecting())
}
