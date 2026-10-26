//! C ABI for `libobs/obs-output-start.c.h`, the output start and
//! begin-capture decisions of `obs-output.c`, declared in
//! `libobs/obs-output-start.h` without `EXPORT`.
//!
//! `obs-output.c.h` (still C) checks the output (`obs_output_valid()`),
//! logs every warning, loads every atomic (`starting`, `delay_active`,
//! `active`, `reconnecting`, `data_active`, `delay_capturing` and
//! `delay_restart_refs`) where it always did and tests the single fields in
//! their `&&` order, waits on the stopping event, calls `info.start`, the
//! service, the delayed start, the encoder initialization,
//! `pair_encoders()`, `hook_data_capture()` and every signal, and passes
//! `info.flags` and the loaded restart count here. So `struct obs_output`
//! is never mirrored. These shims take no lock, call nothing in libobs and
//! dereference no pointer. They stay hidden (`rust-hidden.txt`). The
//! delayed start asks the `obs_output_raw_audio_delay_on` shim in
//! [`super::output_raw_audio`], and the conversion fill the
//! `obs_encoder_size_audio_defaults` shim in [`super::encoder_size`].
//!
//! No intentional differences.

use std::ffi::c_long;

use crate::output_start as core;

/// `bool obs_output_start_service(uint32_t flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_start_service(flags: u32) -> bool {
    core::service(flags)
}

/// `bool obs_output_start_drop_restart_ref(long delay_restart_refs)`.
// `long` is `i64` on Linux and macOS and `i32` on Windows.
#[allow(clippy::useless_conversion)]
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_start_drop_restart_ref(delay_restart_refs: c_long) -> bool {
    core::drop_restart_ref(i64::from(delay_restart_refs))
}

/// `bool obs_output_start_init_video(uint32_t flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_start_init_video(flags: u32) -> bool {
    core::init_video(flags)
}

/// `bool obs_output_start_init_audio(uint32_t flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_start_init_audio(flags: u32) -> bool {
    core::init_audio(flags)
}

/// `bool obs_output_start_reset_raw(uint32_t flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_start_reset_raw(flags: u32) -> bool {
    core::reset_raw(flags)
}

/// `bool obs_output_start_pair(uint32_t flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_start_pair(flags: u32) -> bool {
    core::pair(flags)
}
