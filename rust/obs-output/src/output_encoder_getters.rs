//! Borrowed first-video and indexed audio encoder getters; the output retains ownership.
use crate::output_slots::{SlotKind, first_slot, slot_exists};
use core::ffi::CStr;

pub fn video_encoder<T: Default>(
    _valid: bool,
    _occupied: impl FnOnce() -> u32,
    _read: impl FnOnce(usize) -> T,
    _log: impl FnOnce(&'static CStr),
) -> T {
    T::default()
}

pub fn audio_encoder<T: Default>(
    _valid: bool,
    _idx: usize,
    _read: impl FnOnce(usize) -> T,
    _log: impl FnOnce(&'static CStr),
) -> T {
    T::default()
}
