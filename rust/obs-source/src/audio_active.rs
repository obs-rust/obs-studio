//! Public source audio activity query. The live flag is loaded atomically.
use core::sync::atomic::{AtomicBool, Ordering};

/// Read activity, reporting an absent source once before returning false.
pub fn active(value: Option<&AtomicBool>, log_null: impl FnOnce()) -> bool {
    match value {
        Some(value) => value.load(Ordering::SeqCst),
        None => {
            log_null();
            false
        }
    }
}
