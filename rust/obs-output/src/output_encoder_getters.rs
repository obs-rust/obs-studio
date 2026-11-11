//! Borrowed first-video and indexed audio encoder getters; the output retains ownership.
use crate::output_slots::{SlotKind, first_slot, slot_exists};
use core::ffi::CStr;

pub fn video_encoder<T: Default>(
    valid: bool,
    occupied: impl FnOnce() -> u32,
    read: impl FnOnce(usize) -> T,
    log: impl FnOnce(&'static CStr),
) -> T {
    if !valid {
        log(c"obs_output_get_video_encoder");
        return T::default();
    }
    match first_slot(SlotKind::Video, occupied()) {
        None => T::default(),
        Some(idx) => read(idx),
    }
}

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
