//! Scale-filter storage followed by atomic transform invalidation.
use core::sync::atomic::{AtomicBool, Ordering};

/// Preserve all enum bits, even unknown values, and invalidate on every call.
/// A missing item reports the original pointer-validation diagnostic.
pub fn set(item: Option<(&mut u32, &AtomicBool)>, filter: u32, invalid: impl FnOnce()) {
    if let Some((stored, dirty)) = item {
        *stored = filter;
        dirty.swap(true, Ordering::SeqCst);
    } else {
        invalid();
    }
}
