//! C ABI for `libobs/audio-monitoring/osx/coreaudio-pacing.c.h`.
//!
//! The CoreAudio monitor (`osx/coreaudio-output.c`) calls these inside
//! libobs on macOS. The C file has no `EXPORT`, so the symbols stay hidden
//! (`rust-hidden.d`). The seven functions keep the C signatures: `size_t` is
//! `usize`, `uint32_t` is `u32`, `float` is `f32` and `bool` is `bool`.
//!
//! Only macOS calls them. Elsewhere the shims are built but unused, and
//! hidden all the same.

use crate::coreaudio_pacing::{
    all_back, buffer_size, can_fill, needs_volume, recover_failed_enqueue, wait_over, wait_size,
};

/// Same contract as C `coreaudio_monitor_buffer_size`.
#[unsafe(no_mangle)]
pub extern "C" fn coreaudio_monitor_buffer_size(channels: u32, rate: u32) -> usize {
    buffer_size(channels, rate)
}

/// Same contract as C `coreaudio_monitor_wait_size`.
#[unsafe(no_mangle)]
pub extern "C" fn coreaudio_monitor_wait_size(buffer_size: usize) -> usize {
    wait_size(buffer_size)
}

/// Same contract as C `coreaudio_monitor_can_fill`.
#[unsafe(no_mangle)]
pub extern "C" fn coreaudio_monitor_can_fill(queued: usize, buffer_size: usize) -> bool {
    can_fill(queued, buffer_size)
}

/// Same contract as C `coreaudio_monitor_needs_volume`.
#[unsafe(no_mangle)]
pub extern "C" fn coreaudio_monitor_needs_volume(vol: f32) -> bool {
    needs_volume(vol)
}

/// Same contract as C `coreaudio_monitor_wait_over`.
#[unsafe(no_mangle)]
pub extern "C" fn coreaudio_monitor_wait_over(queued: usize, wait_size: usize) -> bool {
    wait_over(queued, wait_size)
}

/// Same contract as C `coreaudio_monitor_all_back`.
#[unsafe(no_mangle)]
pub extern "C" fn coreaudio_monitor_all_back(empty_bytes: usize, ptr_size: usize) -> bool {
    all_back(empty_bytes, ptr_size)
}

/// C `coreaudio_monitor_recover_failed_enqueue`, with the port's
/// intentional difference: yes, where the C says no.
#[unsafe(no_mangle)]
pub extern "C" fn coreaudio_monitor_recover_failed_enqueue() -> bool {
    recover_failed_enqueue()
}
