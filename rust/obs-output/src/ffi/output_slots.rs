//! C ABI for `libobs/obs-output-slots.c.h`, the encoder slot rules and
//! setter gates of `obs-output.c`, declared in `libobs/obs-output-slots.h`
//! without `EXPORT`.
//!
//! `obs-output.c.h` (still C) checks the output (`obs_output_valid()`),
//! calls the `log_flag_*` checks in their order, loads `active` where it
//! always did, reads the flags, the slots and the encoder's type, builds the
//! occupancy masks, compares the encoder pointers, takes and releases every
//! reference, hooks the encoders up, creates and destroys the caption
//! tracks, sets the scaled size and logs, and passes plain values here. So
//! `struct obs_output` and `struct obs_encoder` are never mirrored, and
//! these shims take no lock and call nothing in libobs. The only pointer is
//! the index the two `first` shims write. They stay hidden
//! (`rust-hidden.txt`).
//!
//! Intentional difference: [`obs_output_slots_mixer_mask`] with an index at
//! or past the width of `size_t`, where the C's shift is undefined, gives 0.
//! `obs_output_set_mixer()` asks only below `MAX_AUDIO_MIXES`.

use std::os::raw::c_int;

use crate::output_slots::{self as core, SlotKind};

/// Writes the slot `core::first_slot()` finds for `kind` to `*index`.
///
/// # Safety
///
/// `index` is valid for a write when a slot is found.
unsafe fn write_first(kind: SlotKind, occupied: u32, index: *mut usize) -> bool {
    match core::first_slot(kind, occupied) {
        Some(i) => {
            // SAFETY: the caller's contract; get_first_*_encoder_index()
            // passes a local and returns before calling with NULL.
            unsafe { index.write(i) };
            true
        }
        None => false,
    }
}

/// `bool obs_output_slots_first_video(uint32_t occupied, size_t *index)`.
///
/// # Safety
///
/// As the C: `index` points to a writable `size_t` (written only when a
/// slot is set).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_slots_first_video(occupied: u32, index: *mut usize) -> bool {
    // SAFETY: forwarded from this function's contract.
    unsafe { write_first(SlotKind::Video, occupied, index) }
}

/// `bool obs_output_slots_first_audio(uint32_t occupied, size_t *index)`.
///
/// # Safety
///
/// As the C: `index` points to a writable `size_t` (written only when a
/// slot is set).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_slots_first_audio(occupied: u32, index: *mut usize) -> bool {
    // SAFETY: forwarded from this function's contract.
    unsafe { write_first(SlotKind::Audio, occupied, index) }
}

/// `bool obs_output_slots_mixer_in_range(size_t mixer_idx)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_slots_mixer_in_range(mixer_idx: usize) -> bool {
    core::mixer_in_range(mixer_idx)
}

/// `size_t obs_output_slots_mixer_mask(size_t mixer_idx)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_slots_mixer_mask(mixer_idx: usize) -> usize {
    core::mixer_mask(mixer_idx)
}

/// `bool obs_output_slots_video_type_ok(int encoder_type)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_slots_video_type_ok(encoder_type: c_int) -> bool {
    core::type_ok(SlotKind::Video, encoder_type)
}

/// `bool obs_output_slots_audio_type_ok(int encoder_type)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_slots_audio_type_ok(encoder_type: c_int) -> bool {
    core::type_ok(SlotKind::Audio, encoder_type)
}

/// `bool obs_output_slots_video_index_ok(uint32_t flags, size_t idx)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_slots_video_index_ok(flags: u32, idx: usize) -> bool {
    core::index_ok(SlotKind::Video, flags, idx)
}

/// `bool obs_output_slots_audio_index_ok(uint32_t flags, size_t idx)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_slots_audio_index_ok(flags: u32, idx: usize) -> bool {
    core::index_ok(SlotKind::Audio, flags, idx)
}

/// `bool obs_output_slots_default_index(size_t idx)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_slots_default_index(idx: usize) -> bool {
    core::default_index(idx)
}

/// `bool obs_output_slots_scaled_axis_set(uint32_t size)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_slots_scaled_axis_set(size: u32) -> bool {
    core::scaled_axis_set(size)
}

/// `bool obs_output_slots_video_slot(size_t idx)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_slots_video_slot(idx: usize) -> bool {
    core::slot_exists(SlotKind::Video, idx)
}

/// `bool obs_output_slots_audio_slot(size_t idx)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_slots_audio_slot(idx: usize) -> bool {
    core::slot_exists(SlotKind::Audio, idx)
}
