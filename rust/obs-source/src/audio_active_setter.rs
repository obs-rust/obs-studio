//! Atomic source activity update. The signal primitive owns calldata and delivery.
use core::sync::atomic::{AtomicBool, Ordering};
use crate::monitoring::Active;

/// Update before notifying observers, suppressing repeated values.
/// A missing source logs exactly once and never invokes the signal primitive.
pub fn set_active(
    field: Option<&AtomicBool>,
    active: bool,
    invalid: impl FnOnce(),
    signal: impl FnOnce(Active),
) {
    let _ = (field, active, invalid, signal);
    todo!()
}
