//! Field readers from `libobs/obs-hotkey-binding-getters.c.h`.
//! Stored values are returned without normalization or resolving the hotkey ID.
use crate::nix_hotkey_state::KeyCombination;

/// Copy the stored combination, preserving all modifier bits.
pub fn key_combination(value: &KeyCombination) -> KeyCombination {
    *value
}
/// Read the binding's own ID independently of its hotkey.
pub fn hotkey_id(value: &usize) -> usize {
    *value
}
/// Return the stored handle without dereferencing or retaining it.
pub fn hotkey<H: Copy>(value: &H) -> H {
    *value
}
