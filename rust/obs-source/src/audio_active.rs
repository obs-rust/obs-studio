//! Public source audio activity query. The live flag is loaded atomically.
use core::sync::atomic::AtomicBool;

/// Read activity, reporting an absent source once before returning false.
pub fn active(value: Option<&AtomicBool>, log_null: impl FnOnce()) -> bool {
    let _ = (value, log_null);
    todo!("port source audio activity")
}
