//! Atomic source activity update. The signal primitive owns calldata and delivery.
use crate::monitoring::Active;
use core::sync::atomic::{AtomicBool, Ordering};

/// Update before notifying observers, suppressing repeated values.
/// A missing source logs exactly once and never invokes the signal primitive.
pub fn set_active(
    field: Option<&AtomicBool>,
    active: bool,
    invalid: impl FnOnce(),
    signal: impl FnOnce(Active),
) {
    let Some(field) = field else {
        invalid();
        return;
    };
    let previous = field.swap(active, Ordering::SeqCst);
    let action = crate::monitoring::audio_active(previous, active);
    if action != Active::None {
        signal(action);
    }
}
