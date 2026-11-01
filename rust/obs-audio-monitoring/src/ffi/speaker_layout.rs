//! C ABI for `libobs/audio-monitoring/monitor-speaker-layout.c.h`.
//!
//! The WASAPI monitor's `audio_monitor_init_wasapi()`
//! (`win32/wasapi-output.c`) calls it inside libobs on Windows. The C file
//! has no `EXPORT`, so the symbol stays hidden (`rust-hidden.d`). `enum
//! speaker_layout` crosses as `c_int`, the raw value: 0 to 6 or 8.
//!
//! Only Windows calls it. Elsewhere the shim is built but unused, and hidden
//! all the same.

use std::ffi::c_int;

use crate::speaker_layout::speaker_layout;

/// Same contract as C `audio_monitor_speaker_layout`.
#[unsafe(no_mangle)]
pub extern "C" fn audio_monitor_speaker_layout(mask: u32, channels: u32) -> c_int {
    speaker_layout(mask, channels)
}
