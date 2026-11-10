//! Borrowed indexed video encoder lookup; the output retains ownership.
use crate::output_slots::{SlotKind, slot_exists};
use core::ffi::CStr;

pub fn video_encoder<T: Default>(
    valid: bool,
    idx: usize,
    read: impl FnOnce(usize) -> T,
    log: impl FnOnce(&'static CStr),
) -> T {
    let _ = (valid, idx, read, log);
    todo!()
}
