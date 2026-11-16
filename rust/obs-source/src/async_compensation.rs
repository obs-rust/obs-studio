//! Async compensation is a request flag, consumed by the next audio packet.
//!
//! The setter takes neither `async_mutex` nor `audio_buf_mutex`. It must not
//! rebuild the resampler or update `last_async_compensation`: the serialized
//! audio input path observes the atomic flag and does that on its next packet.
//! `audio_buf_mutex` protects shared timing (including the video's reanchor and
//! resample offset); `async_mutex` protects the frame queue. The graphics path
//! may take the former inside the latter, so adding locks here is unnecessary.
//!
//! Timestamp arithmetic belongs to the existing timing paths, not this setter:
//! unsigned sums/subtractions wrap modulo 2^64, signed sync offsets convert to
//! u64 before addition, and timestamp distance uses ordered subtraction rather
//! than modular distance. Zero remains the no-frame/no-next-packet sentinel.
//! Toggling compensation preserves all of that state, even at u64 boundaries.
use core::sync::atomic::{AtomicBool, Ordering};

/// Store the request without changing timing state or rebuilding the resampler.
pub fn set(value: Option<&AtomicBool>, compensate: bool, log_null: impl FnOnce()) {
    match value {
        Some(value) => value.store(compensate, Ordering::SeqCst),
        None => log_null(),
    }
}
