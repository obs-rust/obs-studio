//! Safe core of `libobs/obs-output-encoder-getters.c.h`: the borrowed
//! first video encoder and indexed audio encoder of an output.
//!
//! `obs-output.c` keeps `obs_output_valid()`'s pointer check and every read
//! of the output's slots; the shim passes the check's result, reads each
//! slot through the live adapter `obs-output-encoder-getters-fields.c.h`,
//! and returns what `obs_output_get_video_encoder2()` gives. Neither getter
//! takes a lock, as in C, and neither returns a reference: the output keeps
//! the encoders it holds.

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

use crate::output_slots::{SlotKind, first_slot, slot_exists};
use core::ffi::CStr;

/// `obs_output_get_video_encoder()`: for a valid output, the occupancy mask
/// of its `slots` video slots, built as `get_first_video_encoder_index()`
/// builds it (every slot read, in order; bit `i` for an occupied slot `i`,
/// none from the mask's width up), then `get` of the first occupied slot
/// (`obs_output_get_video_encoder2()`), or the default when none is. A NULL
/// output (`valid == false`) logs and reads nothing.
pub fn video_encoder<T: Default>(
    valid: bool,
    slots: usize,
    mut occupied: impl FnMut(usize) -> bool,
    get: impl FnOnce(usize) -> T,
    log: impl FnOnce(&'static CStr),
) -> T {
    if !valid {
        log(c"obs_output_get_video_encoder");
        return T::default();
    }
    let mut mask = 0_u32;
    for i in 0..slots {
        if occupied(i) {
            mask |= u32::try_from(i)
                .ok()
                .and_then(|i| 1_u32.checked_shl(i))
                .unwrap_or(0);
        }
    }
    match first_slot(SlotKind::Video, mask) {
        Some(first) => get(first),
        None => T::default(),
    }
}

/// `obs_output_get_audio_encoder()`: for a valid output, `read` of audio
/// slot `idx` when `idx` is one of the slots (whatever the flags), else the
/// default. A NULL output logs and reads nothing.
pub fn audio_encoder<T: Default>(
    valid: bool,
    idx: usize,
    read: impl FnOnce(usize) -> T,
    log: impl FnOnce(&'static CStr),
) -> T {
    if !valid {
        log(c"obs_output_get_audio_encoder");
        return T::default();
    }
    if !slot_exists(SlotKind::Audio, idx) {
        return T::default();
    }
    read(idx)
}
