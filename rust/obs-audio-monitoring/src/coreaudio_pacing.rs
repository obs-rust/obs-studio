//! Safe core of `libobs/audio-monitoring/osx/coreaudio-pacing.c.h`: the
//! queue pacing of the CoreAudio monitor (`osx/coreaudio-output.c`).
//!
//! The monitor plays through an AudioQueue of [`BUFFERS`] buffers. A free
//! buffer is filled whenever a whole buffer of audio is queued, the queue
//! pauses once every buffer has come back unfilled, and it starts again when
//! the wait size is queued:
//!
//! - `audio_monitor_init()`: [`buffer_size`] and [`wait_size`];
//! - `fill_buffer()`: [`can_fill`], and after a failed
//!   `AudioQueueEnqueueBuffer()`, [`recover_failed_enqueue`];
//! - `on_audio_playback()`: [`needs_volume`] and [`wait_over`];
//! - `coreaudio_buffer_audio()`: [`all_back`].
//!
//! `coreaudio-output.c` keeps both deques (every push and pop), the mutex,
//! every AudioQueue call, the resample, the volume loop, the logs, the fill
//! loops and the paused flag, and reads each size where it did. These keep
//! no state, take no lock and call nothing in CoreAudio or libobs.

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

/// `COREAUDIO_MONITOR_BUFFERS`: the AudioQueue buffers a monitor has.
pub const BUFFERS: usize = 3;

/// Bytes in one AudioQueue buffer: 30 ms of packed float audio,
/// `channels * sizeof(float) * rate / 100 * 3` in `size_t`.
#[must_use]
pub fn buffer_size(channels: u32, rate: u32) -> usize {
    // Left to right in size_t, as the C: the bytes of one second, divided
    // by 100 and truncated, then times 3. Each product wraps.
    usize_from(channels)
        .wrapping_mul(FLOAT_BYTES)
        .wrapping_mul(usize_from(rate))
        .wrapping_div(100)
        .wrapping_mul(3)
}

/// Bytes queued before a paused queue starts again: every buffer's worth.
#[must_use]
pub fn wait_size(buffer_size: usize) -> usize {
    buffer_size.wrapping_mul(BUFFERS)
}

/// Whether `queued` bytes fill a buffer.
#[must_use]
pub fn can_fill(queued: usize, buffer_size: usize) -> bool {
    queued >= buffer_size
}

/// Whether the volume step runs: the volume is not 1 within `EPSILON`.
#[must_use]
pub fn needs_volume(vol: f32) -> bool {
    crate::pulse_monitor::needs_volume(vol)
}

/// Whether the wait is over once `queued` bytes are queued.
#[must_use]
pub fn wait_over(queued: usize, wait_size: usize) -> bool {
    queued >= wait_size
}

/// Whether every buffer is back on the free list, which holds
/// `empty_bytes` of buffer pointers of `ptr_size` bytes.
#[must_use]
pub fn all_back(empty_bytes: usize, ptr_size: usize) -> bool {
    empty_bytes == ptr_size.wrapping_mul(BUFFERS)
}

/// After `AudioQueueEnqueueBuffer()` failed and the queue was stopped:
/// whether the buffer goes back on the free list and the monitor takes the
/// paused state with [`wait_size`], so that later audio starts the queue
/// again.
#[must_use]
pub fn recover_failed_enqueue() -> bool {
    // The intentional difference: the C keeps the buffer out for good, so
    // only two buffers can come back, the queue never pauses and is never
    // started again.
    true
}

/// `sizeof(float)`.
const FLOAT_BYTES: usize = 4;

/// A `uint32_t` as `size_t`, as the C's conversions. `usize` is at least 32
/// bits on every host libobs builds for.
fn usize_from(v: u32) -> usize {
    usize::try_from(v).unwrap_or(usize::MAX)
}
