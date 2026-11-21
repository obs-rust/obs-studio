//! Safe core of `libobs/obs-output-output-caption-text1.c.h`:
//! `obs_output_output_caption_text1()`, the legacy caption text call.
//!
//! A missing output emits `obs_output_valid()`'s diagnostic and forwards
//! nothing. A present output forwards once, with the caller's output and text
//! unchanged (a NULL text included), to `obs_output_output_caption_text2()`
//! with [`DISPLAY_DURATION`]. The text is never read here.

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

/// Seconds each page is shown: the C's `(double)2.0f`, exactly 2.0.
pub const DISPLAY_DURATION: f64 = 2.0;

/// `None` is the NULL output: `log_null` runs once and nothing is
/// forwarded. Otherwise `forward` runs once with the output and
/// [`DISPLAY_DURATION`].
pub fn caption_text1<O>(output: Option<O>, log_null: impl FnOnce(), forward: impl FnOnce(O, f64)) {
    let Some(output) = output else {
        log_null();
        return;
    };
    forward(output, DISPLAY_DURATION);
}
