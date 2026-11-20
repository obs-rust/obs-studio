//! Safe core of `libobs/obs-output-get-last-error.c.h`: which stored error
//! `obs_output_get_last_error()` borrows.
//!
//! The output's own error comes first, then each video encoder slot in
//! ascending order, then each audio encoder slot in ascending order. The
//! first present error is returned unchanged; an empty or non-UTF-8 string
//! is an error like any other. Every read is lazy, so nothing below a hit
//! is read. The core takes no lock, copies nothing and keeps nothing.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

use crate::output_slots::{MAX_OUTPUT_AUDIO_ENCODERS, MAX_OUTPUT_VIDEO_ENCODERS};

/// The C-owned fields of one valid output, each read only when asked.
pub trait LastErrorFields {
    /// A present encoder.
    type Encoder;
    /// A present, borrowed error string.
    type Error;
    /// The output's own `last_error_message`.
    fn own(&mut self) -> Option<Self::Error>;
    /// The encoder in video slot `idx`, below `MAX_OUTPUT_VIDEO_ENCODERS`.
    fn video(&mut self, idx: usize) -> Option<Self::Encoder>;
    /// The encoder in audio slot `idx`, below `MAX_OUTPUT_AUDIO_ENCODERS`.
    fn audio(&mut self, idx: usize) -> Option<Self::Encoder>;
    /// A present encoder's `last_error_message`.
    fn encoder_error(&mut self, encoder: Self::Encoder) -> Option<Self::Error>;
}

/// `None` for the output is the NULL output: `log_null` emits
/// `obs_output_valid()`'s diagnostic and no field is read.
pub fn last_error<F: LastErrorFields>(
    output: Option<&mut F>,
    log_null: impl FnOnce(),
) -> Option<F::Error> {
    let Some(output) = output else {
        log_null();
        return None;
    };
    if let Some(error) = output.own() {
        return Some(error);
    }
    for idx in 0..MAX_OUTPUT_VIDEO_ENCODERS {
        if let Some(error) = output.video(idx).and_then(|e| output.encoder_error(e)) {
            return Some(error);
        }
    }
    for idx in 0..MAX_OUTPUT_AUDIO_ENCODERS {
        if let Some(error) = output.audio(idx).and_then(|e| output.encoder_error(e)) {
            return Some(error);
        }
    }
    None
}
