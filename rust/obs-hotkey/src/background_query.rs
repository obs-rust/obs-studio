//! Whether a combination can be handled by background hotkeys.

/// The native state accepts every combination without asking for an identifier.
pub const NATIVE: i32 = 0;

/// Query the identifier only when the desktop owns background shortcuts.
pub fn is_background(_state: i32, _lookup: impl FnOnce() -> bool) -> bool {
    todo!("port background combination query")
}
