//! Async compensation is a request flag, consumed by the next audio packet.
use core::sync::atomic::AtomicBool;

/// Store the request without changing timing state or rebuilding the resampler.
pub fn set(value: Option<&AtomicBool>, compensate: bool, log_null: impl FnOnce()) {
    let _ = (value, compensate, log_null);
    todo!("port obs_source_set_async_compensation")
}
