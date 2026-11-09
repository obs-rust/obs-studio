//! Output active query independent of the private C layout.

/// Preserve the NULL default and lazy reconnecting query.
pub fn get(active: Option<bool>, reconnecting: impl FnOnce() -> bool) -> bool {
    let _ = (active, reconnecting);
    todo!("port obs_output_active")
}
